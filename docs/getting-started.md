# Getting started

Start the complete local stack and create your first queue.

## What you need

- Docker with Docker Compose
- Just 1.45 or newer
- `curl`

Check the tools, then start Retsu:

```console
just doctor
just local-up
```

This starts the API, workers, PostgreSQL, cache, and monitoring tools. Run `just local-ready` whenever you want to check them.

## Create a queue

The API is available at <http://127.0.0.1:2424>.

```bash
curl --request POST http://127.0.0.1:2424/v1/queues \
  --header 'content-type: application/json' \
  --data '{"name":"getting-started"}'
```

The response contains the queue ID. Use it with the [Queue API](queues.md) to add, receive, and complete a message.

## Watch it work

Open the [showcase dashboard](http://127.0.0.1:24246/d/retsu-showcase/retsu-showcase), then run:

```console
just local-showcase
```

The five-minute demonstration shows priority handling, retries, expiry, and dead-letter cleanup.

## Stop the stack

```console
just local-stop
```

This keeps local data. Use `just stack-down` to remove the containers while keeping their persisted volumes.
