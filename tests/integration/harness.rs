use std::{
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
use sqlx::{PgPool, Postgres as SqlxPostgres, pool::PoolConnection, postgres::PgPoolOptions};
use tempfile::TempDir;
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{
        ContainerAsync, GenericImage, ImageExt,
        core::{IntoContainerPort, WaitFor},
        runners::AsyncRunner,
    },
};
use uuid::Uuid;

const PROCESS_START_TIMEOUT: Duration = Duration::from_secs(20);
const POSTGRES_PORT: u16 = 5432;
const DISTRIBUTED_CACHE_PORT: u16 = 6379;
const DRAGONFLY_IMAGE: &str = "docker.dragonflydb.io/dragonflydb/dragonfly";
const DRAGONFLY_TAG: &str = "v1.38.0";
pub const DEQUEUE_PAUSE_LOCK_KEY: i64 = 363_636;
pub const DEQUEUE_PAUSE_PAYLOAD: &str = "integration-pause-dequeue";

pub struct IntegrationSystem {
    processes: Vec<ManagedProcess>,
    database_pool: PgPool,
    _postgres: ContainerAsync<Postgres>,
    _distributed_cache: ContainerAsync<GenericImage>,
    log_directory: TempDir,
    database_url: String,
    distributed_cache_url: String,
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

#[derive(Debug, Deserialize)]
pub struct QueueDetails {
    pub id: Uuid,
    pub name: String,
    pub visibility_timeout_seconds: u32,
    pub max_delivery_attempts: u16,
    pub default_message_ttl_seconds: u32,
}

#[derive(Deserialize)]
struct CreateQueueResponse {
    id: Uuid,
}

#[derive(Deserialize)]
struct EnqueueMessageResponse {
    id: Uuid,
}

impl IntegrationSystem {
    pub async fn start() -> anyhow::Result<Self> {
        let log_directory =
            tempfile::tempdir().context("failed to create integration-test log directory")?;

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

        let distributed_cache = GenericImage::new(DRAGONFLY_IMAGE, DRAGONFLY_TAG)
            .with_exposed_port(DISTRIBUTED_CACHE_PORT.tcp())
            .with_wait_for(WaitFor::healthcheck())
            .with_cmd([
                "--cache_mode=true",
                "--maxmemory=256mb",
                "--proactor_threads=1",
                "--primary_port_http_enabled=false",
            ])
            .start()
            .await
            .context("failed to start Dragonfly through Testcontainers")?;
        let distributed_cache_host = distributed_cache
            .get_host()
            .await
            .context("failed to resolve the distributed-cache container host")?;
        let distributed_cache_port = distributed_cache
            .get_host_port_ipv4(DISTRIBUTED_CACHE_PORT)
            .await
            .context("failed to resolve the distributed-cache container port")?;
        let distributed_cache_url =
            format!("redis://{distributed_cache_host}:{distributed_cache_port}");

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
            &distributed_cache_url,
            &[("RETSU_HTTP__PORT", api_port.to_string())],
            log_directory.path(),
        )?;

        wait_for_http(&client, &format!("{api_base_url}/health/ready"), &mut api).await?;

        Ok(Self {
            processes: vec![api],
            database_pool,
            _postgres: postgres,
            _distributed_cache: distributed_cache,
            log_directory,
            database_url,
            distributed_cache_url,
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
            &self.distributed_cache_url,
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

    pub async fn update_queue(
        &self,
        queue_id: Uuid,
        visibility_timeout_seconds: Option<u32>,
        max_delivery_attempts: Option<u16>,
        default_message_ttl_seconds: Option<u32>,
    ) -> anyhow::Result<QueueDetails> {
        let response = self
            .client
            .patch(format!("{}/v1/queues/{queue_id}", self.api_base_url))
            .json(&json!({
                "visibility_timeout_seconds": visibility_timeout_seconds,
                "max_delivery_attempts": max_delivery_attempts,
                "default_message_ttl_seconds": default_message_ttl_seconds,
            }))
            .send()
            .await
            .context("queue update request failed")?;

        let body = expect_body(response, StatusCode::OK, "update queue").await?;
        serde_json::from_str(&body).context("update queue response was not valid JSON")
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

    pub async fn message_ttl_seconds(&self, message_id: Uuid) -> anyhow::Result<f64> {
        sqlx::query_scalar(
            r#"
            SELECT EXTRACT(
                EPOCH FROM (expires_at - enqueued_at)
            )::DOUBLE PRECISION
            FROM queue_message
            WHERE id = $1
            "#,
        )
        .bind(message_id)
        .fetch_one(&self.database_pool)
        .await
        .context("failed to inspect persisted message TTL")
    }

    pub async fn insert_queue(
        &self,
        name: &str,
        visibility_timeout_seconds: u32,
        max_delivery_attempts: u16,
        default_message_ttl_seconds: u32,
    ) -> anyhow::Result<Uuid> {
        let queue_id = Uuid::now_v7();

        sqlx::query(
            r#"
            INSERT INTO queue (
                id,
                name,
                visibility_timeout_seconds,
                max_delivery_attempts,
                default_message_ttl_seconds
            )
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(queue_id)
        .bind(name)
        .bind(i32::try_from(visibility_timeout_seconds)?)
        .bind(i16::try_from(max_delivery_attempts)?)
        .bind(i32::try_from(default_message_ttl_seconds)?)
        .execute(&self.database_pool)
        .await
        .context("failed to insert an integration-test queue")?;

        Ok(queue_id)
    }

    pub async fn distributed_queue_details(
        &self,
        queue_id: Uuid,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let client = redis::Client::open(self.distributed_cache_url.as_str())
            .context("failed to configure the integration-test distributed-cache client")?;
        let mut connection = client
            .get_connection_manager()
            .await
            .context("failed to connect to the integration-test distributed cache")?;
        let value: Option<Vec<u8>> = redis::cmd("GET")
            .arg(format!("retsu:queue_details:{queue_id}"))
            .query_async(&mut connection)
            .await
            .context("failed to read queue details from the distributed cache")?;

        value
            .map(|value| {
                serde_json::from_slice(&value)
                    .context("distributed queue details were not valid JSON")
            })
            .transpose()
    }

    pub async fn install_dequeue_pause_trigger(&self) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE FUNCTION integration_pause_selected_dequeue()
            RETURNS TRIGGER
            LANGUAGE PLPGSQL
            AS $$
            BEGIN
                IF convert_from(NEW.payload, 'UTF8')
                    = 'integration-pause-dequeue'
                THEN
                    PERFORM pg_advisory_xact_lock(363636);
                END IF;

                RETURN NULL;
            END;
            $$
            "#,
        )
        .execute(&self.database_pool)
        .await
        .context("failed to install the dequeue pause function")?;

        sqlx::query(
            r#"
            CREATE TRIGGER integration_pause_selected_dequeue_after_update
            AFTER UPDATE OF receipt_handle ON queue_message
            FOR EACH ROW
            WHEN (OLD.receipt_handle IS DISTINCT FROM NEW.receipt_handle)
            EXECUTE FUNCTION integration_pause_selected_dequeue()
            "#,
        )
        .execute(&self.database_pool)
        .await
        .context("failed to install the dequeue pause trigger")?;

        Ok(())
    }

    pub async fn hold_advisory_lock(
        &self,
        lock_key: i64,
    ) -> anyhow::Result<PoolConnection<SqlxPostgres>> {
        let mut connection = self
            .database_pool
            .acquire()
            .await
            .context("failed to acquire the advisory-lock test connection")?;

        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(lock_key)
            .execute(&mut *connection)
            .await
            .context("failed to hold the dequeue pause advisory lock")?;

        connection.close_on_drop();

        Ok(connection)
    }

    pub async fn wait_for_advisory_lock_waiter(&self) -> anyhow::Result<()> {
        eventually(
            "the selected dequeue to wait on the test advisory lock",
            Duration::from_secs(5),
            || async {
                let waiting = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT EXISTS (
                        SELECT 1
                        FROM pg_stat_activity
                        WHERE datname = CURRENT_DATABASE()
                          AND wait_event_type = 'Lock'
                          AND wait_event = 'advisory'
                    )
                    "#,
                )
                .fetch_one(&self.database_pool)
                .await
                .context("failed to inspect advisory-lock waiters")?;

                Ok(waiting.then_some(()))
            },
        )
        .await
    }

    pub async fn release_advisory_lock(
        &self,
        connection: &mut PoolConnection<SqlxPostgres>,
        lock_key: i64,
    ) -> anyhow::Result<()> {
        let unlocked = sqlx::query_scalar::<_, bool>("SELECT pg_advisory_unlock($1)")
            .bind(lock_key)
            .fetch_one(&mut **connection)
            .await
            .context("failed to release the dequeue pause advisory lock")?;

        ensure!(unlocked, "the dequeue pause advisory lock was not held");
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
    let output = base_command(database_url, None)
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
    distributed_cache_url: &str,
    environment: &[(&str, String)],
    log_directory: &Path,
) -> anyhow::Result<ManagedProcess> {
    let log_path = log_directory.join(format!("{name}-{}.log", Uuid::new_v4().simple()));
    let stdout = File::create(&log_path)
        .with_context(|| format!("failed to create process log {}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("failed to clone process log {}", log_path.display()))?;

    let mut command = base_command(database_url, Some(distributed_cache_url));
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

fn base_command(database_url: &str, distributed_cache_url: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_retsu"));

    command
        .arg("--config")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("config/retsu.yaml"))
        .env("RETSU_ENVIRONMENT", "test")
        .env("RETSU_DATABASE__URL", database_url)
        .env("RETSU_LOGGING__FILTER", "info")
        .env("RETSU_TELEMETRY__TRACES__ENABLED", "false")
        .env("RETSU_WORKER__SHUTDOWN_TIMEOUT_SECONDS", "2");

    if let Some(distributed_cache_url) = distributed_cache_url {
        command.env("RETSU_CACHE__DISTRIBUTED__URL", distributed_cache_url);
    }

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
