use std::{
    env,
    fs::{File, read_to_string},
    future::Future,
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context as _, ensure};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tempfile::TempDir;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};
use uuid::Uuid;

const PROCESS_START_TIMEOUT: Duration = Duration::from_secs(20);
const POSTGRES_PORT: u16 = 5432;

pub struct IntegrationSystem {
    processes: Vec<ManagedProcess>,
    database_pool: PgPool,
    _postgres: Option<ContainerAsync<Postgres>>,
    log_directory: TempDir,
    database_url: String,
    api_base_url: String,
    client: Client,
}

pub struct WorkerEndpoint {
    process_index: usize,
    base_url: String,
}

#[derive(Debug, Deserialize)]
pub struct DequeuedMessage {
    pub id: Uuid,
    pub payload: String,
    pub priority: String,
    pub receipt_handle: Uuid,
    pub delivery_attempts: u16,
}

#[derive(Deserialize)]
struct CreateQueueResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct EnqueueMessageResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct ProblemDetails {
    code: String,
}

impl IntegrationSystem {
    pub async fn start() -> anyhow::Result<Self> {
        let log_directory =
            tempfile::tempdir().context("failed to create integration-test log directory")?;

        let (database_url, postgres) =
            if let Ok(database_url) = env::var("RETSU_BENCHMARK_DATABASE_URL") {
                reset_external_benchmark_database(&database_url).await?;
                (database_url, None)
            } else {
                let postgres = Postgres::default()
                    .with_tag("18.4-alpine")
                    .start()
                    .await
                    .context("failed to start PostgreSQL through Testcontainers")?;

                let host = postgres
                    .get_host()
                    .await
                    .context("failed to resolve the PostgreSQL container host")?;
                let port = postgres
                    .get_host_port_ipv4(POSTGRES_PORT)
                    .await
                    .context("failed to resolve the PostgreSQL container port")?;
                let database_url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

                (database_url, Some(postgres))
            };

        run_migrations(&database_url)?;

        let database_pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .context("failed to connect the integration-test database observer")?;

        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .context("failed to build the integration-test HTTP client")?;

        let api_port = unused_port()?;
        let api_base_url = format!("http://127.0.0.1:{api_port}");
        let mut api = spawn_retsu(
            "api",
            &["api"],
            &database_url,
            &[("RETSU_HTTP__PORT", api_port.to_string())],
            log_directory.path(),
        )?;

        wait_for_http(&client, &format!("{api_base_url}/health/ready"), &mut api).await?;

        Ok(Self {
            processes: vec![api],
            database_pool,
            _postgres: postgres,
            log_directory,
            database_url,
            api_base_url,
            client,
        })
    }

    pub async fn start_worker(&mut self, name: &str) -> anyhow::Result<WorkerEndpoint> {
        let management_port = unused_port()?;
        let base_url = format!("http://127.0.0.1:{management_port}");
        let mut process = spawn_retsu(
            name,
            &["worker", "run", "queue", name],
            &self.database_url,
            &[(
                "RETSU_WORKER__MANAGEMENT__PORT",
                management_port.to_string(),
            )],
            self.log_directory.path(),
        )?;

        wait_for_http(
            &self.client,
            &format!("{base_url}/health/ready"),
            &mut process,
        )
        .await?;

        let process_index = self.processes.len();
        self.processes.push(process);

        Ok(WorkerEndpoint {
            process_index,
            base_url,
        })
    }

    pub fn stop_worker(&mut self, worker: &WorkerEndpoint) -> anyhow::Result<()> {
        let process = self
            .processes
            .get_mut(worker.process_index)
            .context("worker process handle was not registered")?;

        process.stop()
    }

    pub async fn create_queue(
        &self,
        name: &str,
        visibility_timeout_seconds: u32,
        max_delivery_attempts: u16,
        default_message_ttl_seconds: u32,
    ) -> anyhow::Result<Uuid> {
        let response = self
            .client
            .post(format!("{}/v1/queues", self.api_base_url))
            .json(&json!({
                "name": name,
                "visibility_timeout_seconds": visibility_timeout_seconds,
                "max_delivery_attempts": max_delivery_attempts,
                "default_message_ttl_seconds": default_message_ttl_seconds,
            }))
            .send()
            .await
            .context("queue creation request failed")?;

        let body = expect_body(response, StatusCode::CREATED, "create queue").await?;
        let response: CreateQueueResponse =
            serde_json::from_str(&body).context("create queue response was not valid JSON")?;

        Ok(response.id)
    }

    pub async fn enqueue_message(
        &self,
        queue_id: Uuid,
        payload: &str,
        priority: &str,
        ttl_seconds: Option<u32>,
    ) -> anyhow::Result<Uuid> {
        let response = self
            .client
            .post(format!(
                "{}/v1/queues/{queue_id}/messages",
                self.api_base_url
            ))
            .json(&json!({
                "payload": payload,
                "priority": priority,
                "ttl_seconds": ttl_seconds,
            }))
            .send()
            .await
            .context("message enqueue request failed")?;

        let body = expect_body(response, StatusCode::CREATED, "enqueue message").await?;
        let response: EnqueueMessageResponse =
            serde_json::from_str(&body).context("enqueue response was not valid JSON")?;

        Ok(response.id)
    }

    pub async fn dequeue_message(&self, queue_id: Uuid) -> anyhow::Result<Option<DequeuedMessage>> {
        let response = self
            .client
            .post(format!(
                "{}/v1/queues/{queue_id}/messages/dequeue",
                self.api_base_url
            ))
            .send()
            .await
            .context("message dequeue request failed")?;

        if response.status() == StatusCode::NO_CONTENT {
            return Ok(None);
        }

        let body = expect_body(response, StatusCode::OK, "dequeue message").await?;
        let message = serde_json::from_str(&body).context("dequeue response was not valid JSON")?;

        Ok(Some(message))
    }

    pub async fn acknowledge_message(
        &self,
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> anyhow::Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/v1/queues/{queue_id}/messages/{message_id}/acknowledge",
                self.api_base_url
            ))
            .json(&json!({ "receipt_handle": receipt_handle }))
            .send()
            .await
            .context("message acknowledgement request failed")?;

        expect_status(response, StatusCode::NO_CONTENT, "acknowledge message").await
    }

    pub async fn rejected_acknowledgement_code(
        &self,
        queue_id: Uuid,
        message_id: Uuid,
        receipt_handle: Uuid,
    ) -> anyhow::Result<String> {
        let response = self
            .client
            .post(format!(
                "{}/v1/queues/{queue_id}/messages/{message_id}/acknowledge",
                self.api_base_url
            ))
            .json(&json!({ "receipt_handle": receipt_handle }))
            .send()
            .await
            .context("rejected message acknowledgement request failed")?;

        let body = expect_body(
            response,
            StatusCode::CONFLICT,
            "reject message acknowledgement",
        )
        .await?;
        let problem: ProblemDetails =
            serde_json::from_str(&body).context("acknowledgement error was not valid JSON")?;

        Ok(problem.code)
    }

    pub async fn worker_metrics(&self, worker: &WorkerEndpoint) -> anyhow::Result<String> {
        let response = self
            .client
            .get(format!("{}/metrics", worker.base_url))
            .send()
            .await
            .context("worker metrics request failed")?;

        expect_body(response, StatusCode::OK, "read worker metrics").await
    }

    pub async fn message_exists(&self, message_id: Uuid) -> anyhow::Result<bool> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM queue_message WHERE id = $1)")
            .bind(message_id)
            .fetch_one(&self.database_pool)
            .await
            .context("failed to inspect active message persistence")
    }

    #[allow(dead_code)]
    pub async fn delete_message_directly(&self, message_id: Uuid) -> anyhow::Result<()> {
        let result = sqlx::query("DELETE FROM queue_message WHERE id = $1")
            .bind(message_id)
            .execute(&self.database_pool)
            .await
            .context("failed to delete benchmark message")?;

        ensure!(
            result.rows_affected() == 1,
            "expected to delete one benchmark message, deleted {}",
            result.rows_affected()
        );

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn restore_message_directly(&self, message_id: Uuid) -> anyhow::Result<()> {
        let result = sqlx::query(
            r#"
            UPDATE queue_message
            SET
                state = 'READY',
                delivery_attempts = 0,
                receipt_handle = NULL,
                visibility_deadline = NULL,
                last_delivered_at = NULL
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .execute(&self.database_pool)
        .await
        .context("failed to restore benchmark message")?;

        ensure!(
            result.rows_affected() == 1,
            "expected to restore one benchmark message, restored {}",
            result.rows_affected()
        );

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn reset_dequeue_fixture_directly(
        &self,
        queue_id: Uuid,
        message_count: u32,
        payload_size_bytes: u32,
    ) -> anyhow::Result<()> {
        sqlx::query("TRUNCATE TABLE queue_message, queue_priority_state_shard RESTART IDENTITY")
            .execute(&self.database_pool)
            .await
            .context("failed to clear the benchmark dequeue fixture")?;

        self.seed_ready_messages_directly(queue_id, message_count, payload_size_bytes)
            .await
    }

    #[allow(dead_code)]
    pub async fn seed_ready_messages_directly(
        &self,
        queue_id: Uuid,
        message_count: u32,
        payload_size_bytes: u32,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO queue_message (id, queue_id, payload, priority, expires_at)
            SELECT
                gen_random_uuid(),
                $1,
                repeat('x', $2)::BYTEA,
                2,
                CURRENT_TIMESTAMP + INTERVAL '1 day'
            FROM generate_series(1, $3)
            "#,
        )
        .bind(queue_id)
        .bind(i32::try_from(payload_size_bytes).context("payload size exceeded i32")?)
        .bind(i32::try_from(message_count).context("message count exceeded i32")?)
        .execute(&self.database_pool)
        .await
        .context("failed to seed benchmark messages")?;

        Ok(())
    }

    pub async fn dead_letter_reason(&self, message_id: Uuid) -> anyhow::Result<Option<String>> {
        sqlx::query_scalar("SELECT reason FROM queue_dead_letter_message WHERE id = $1")
            .bind(message_id)
            .fetch_optional(&self.database_pool)
            .await
            .context("failed to inspect dead-letter persistence")
    }
}

async fn reset_external_benchmark_database(database_url: &str) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .context("failed to connect to the external benchmark database")?;
    let database_name: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&pool)
        .await
        .context("failed to inspect the external benchmark database name")?;

    ensure!(
        database_name == "retsu_benchmark",
        "refusing to reset external database `{database_name}`; expected `retsu_benchmark`"
    );

    sqlx::query("DROP SCHEMA public CASCADE")
        .execute(&pool)
        .await
        .context("failed to drop the external benchmark schema")?;
    sqlx::query("CREATE SCHEMA public")
        .execute(&pool)
        .await
        .context("failed to recreate the external benchmark schema")?;
    pool.close().await;

    Ok(())
}

pub fn unique_queue_name(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

pub async fn eventually<T, F, Fut>(
    description: &str,
    timeout: Duration,
    mut probe: F,
) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<Option<T>>>,
{
    let deadline = Instant::now() + timeout;
    let mut last_error = None;

    loop {
        match probe().await {
            Ok(Some(value)) => return Ok(value),
            Ok(None) => {}
            Err(error) => last_error = Some(error.to_string()),
        }

        if Instant::now() >= deadline {
            let suffix = last_error
                .map(|error| format!("; last error: {error}"))
                .unwrap_or_default();

            anyhow::bail!("timed out waiting for {description}{suffix}");
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn run_migrations(database_url: &str) -> anyhow::Result<()> {
    let output = base_command(database_url)
        .args(["migrate"])
        .output()
        .context("failed to start the migration process")?;

    ensure!(
        output.status.success(),
        "migration process failed with {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    Ok(())
}

fn spawn_retsu(
    name: &str,
    arguments: &[&str],
    database_url: &str,
    environment: &[(&str, String)],
    log_directory: &Path,
) -> anyhow::Result<ManagedProcess> {
    let log_path = log_directory.join(format!("{name}-{}.log", Uuid::new_v4().simple()));
    let stdout = File::create(&log_path)
        .with_context(|| format!("failed to create process log {}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("failed to clone process log {}", log_path.display()))?;

    let mut command = base_command(database_url);
    command.args(arguments);

    for (key, value) in environment {
        command.env(key, value);
    }

    let child = command
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("failed to start `{name}`"))?;

    Ok(ManagedProcess {
        name: name.to_owned(),
        child,
        log_path,
    })
}

fn base_command(database_url: &str) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_retsu"));

    command
        .arg("--config")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/retsu.yaml"))
        .env_remove("RETSU_BENCHMARK_DATABASE_URL")
        .env("RETSU_ENVIRONMENT", "test")
        .env("RETSU_DATABASE__URL", database_url)
        .env("RETSU_LOGGING__FILTER", "info")
        .env("RETSU_TELEMETRY__TRACES__ENABLED", "false")
        .env("RETSU_WORKER__SHUTDOWN_TIMEOUT_SECONDS", "2");

    command
}

fn unused_port() -> anyhow::Result<u16> {
    let listener =
        TcpListener::bind(("127.0.0.1", 0)).context("failed to reserve an unused local port")?;
    let port = listener
        .local_addr()
        .context("failed to inspect the reserved local port")?
        .port();

    Ok(port)
}

async fn wait_for_http(
    client: &Client,
    url: &str,
    process: &mut ManagedProcess,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + PROCESS_START_TIMEOUT;

    loop {
        if let Some(status) = process
            .child
            .try_wait()
            .with_context(|| format!("failed to inspect `{}`", process.name))?
        {
            anyhow::bail!(
                "`{}` exited with {status} before becoming ready\n{}",
                process.name,
                process.logs()
            );
        }

        let last_error = match client.get(url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(response) => format!("HTTP {}", response.status()),
            Err(error) => error.to_string(),
        };

        if Instant::now() >= deadline {
            anyhow::bail!(
                "`{}` did not become ready at {url}; last error: {}\n{}",
                process.name,
                last_error,
                process.logs()
            );
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn expect_status(
    response: reqwest::Response,
    expected: StatusCode,
    operation: &str,
) -> anyhow::Result<()> {
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response for {operation}"))?;

    ensure!(
        status == expected,
        "{operation} returned {status}, expected {expected}; body: {body}"
    );

    Ok(())
}

async fn expect_body(
    response: reqwest::Response,
    expected: StatusCode,
    operation: &str,
) -> anyhow::Result<String> {
    let status = response.status();
    let body = response
        .text()
        .await
        .with_context(|| format!("failed to read response for {operation}"))?;

    ensure!(
        status == expected,
        "{operation} returned {status}, expected {expected}; body: {body}"
    );

    Ok(body)
}

struct ManagedProcess {
    name: String,
    child: Child,
    log_path: PathBuf,
}

impl ManagedProcess {
    fn stop(&mut self) -> anyhow::Result<()> {
        if self
            .child
            .try_wait()
            .with_context(|| format!("failed to inspect `{}`", self.name))?
            .is_none()
        {
            self.child
                .kill()
                .with_context(|| format!("failed to stop `{}`", self.name))?;
            self.child
                .wait()
                .with_context(|| format!("failed to reap `{}`", self.name))?;
        }

        Ok(())
    }

    fn logs(&self) -> String {
        read_to_string(&self.log_path).unwrap_or_else(|error| {
            format!(
                "failed to read process log {}: {error}",
                self.log_path.display()
            )
        })
    }
}

impl Drop for ManagedProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
