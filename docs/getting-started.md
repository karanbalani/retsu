# Getting started

Use this guide to start Retsu and its local monitoring tools with Docker.

## What you need

- Docker with Docker Compose.
- Just 1.45 or newer.
- Bash.
- `curl`.

Check that they are ready:

```console
just doctor
```

## Start the complete local stack

Build the production image and start Retsu, PostgreSQL, the distributed cache, and the monitoring services:

```console
just local-up
```

The command applies database migrations, starts the API and all three workers, and waits until the stack is ready.

Check it again at any time:

```console
just local-ready
```

The main local addresses are:

- API: <http://127.0.0.1:2424>
- API readiness: <http://127.0.0.1:2424/health/ready>
- Grafana: <http://127.0.0.1:24246>
- Retsu Showcase dashboard: <http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase>

## Try the queue API

Create a queue:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues \
  --header 'content-type: application/json' \
  --data '{"name":"getting-started"}'
```

The response contains the queue ID needed for message requests. Continue with [Queues and messages](queues.md) to add, receive, and acknowledge a message.

## See Retsu in action

Keep the Showcase dashboard open and run:

```console
just local-showcase
```

The default demonstration lasts five minutes. It creates five queues and shows priority handling, acknowledgements, retries, expiry, and dead-letter cleanup. Pass a whole number from 5 through 20 for a longer run:

```console
just local-showcase 20
```

See [Load testing](load-testing.md) for the other scenarios and the exact showcase workload.

## Stop the stack

Stop services without deleting their data:

```console
just local-stop
```

Use `just stack-down` to remove the containers while keeping persisted data.

## What to read next

- [Local development](local-development.md) explains the host Rust workflow, migrations, and project checks.
- [Configuration](configuration.md) lists every supported runtime setting.
- [Monitoring](observability.md) explains the local dashboards and endpoints.
- The [detailed local infrastructure reference](https://github.com/karanbalani/retsu/blob/main/infra/local/README.md) covers service ports, resource limits, logs, and data reset commands.
