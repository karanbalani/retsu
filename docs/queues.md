# Queues and messages

Retsu can create queues and add messages to them. Reading and processing
messages is not available yet.

## Create a queue

Start the API, then send:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues \
  --header 'content-type: application/json' \
  --data '{"name":"emails"}'
```

A successful request returns status `201` and the new queue:

```json
{
  "id": "019c9a65-7d3a-7c6b-8a9d-123456789abc",
  "name": "emails",
  "visibility_timeout_seconds": 30,
  "max_delivery_attempts": 5
}
```

The `id` will be different for every queue.

### Queue settings

The message-related settings are saved now and will be used when message
processing is added.

| Field | What it controls | Accepted value | Default |
| --- | --- | --- | --- |
| `name` | How the queue is identified | 1–64 lowercase letters, numbers, dots, underscores, or hyphens | Required |
| `visibility_timeout_seconds` | Seconds before an unfinished message can be tried again | 1–21,600 seconds | 30 |
| `max_delivery_attempts` | Times a message can be tried | 1–100 | 5 |

A name must start and end with a letter or number.

To change the optional settings:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues \
  --header 'content-type: application/json' \
  --data '{
    "name": "emails",
    "visibility_timeout_seconds": 60,
    "max_delivery_attempts": 10
  }'
```

### Queue errors

- `400` and `invalid_queue_name`: the name does not follow the rules above.
- `400` and `invalid_visibility_timeout`: the value is outside the allowed
  range.
- `400` and `invalid_max_delivery_attempts`: the value is outside the allowed
  range.
- `409` and `queue_already_exists`: another queue already uses the same name.

## Add a message

Use the queue name in the address:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues/emails/messages \
  --header 'content-type: application/json' \
  --data '{
    "payload": "send welcome email",
    "priority": "HIGH",
    "ttl_seconds": 3600
  }'
```

A successful request returns status `201` and the new message ID:

```json
{
  "id": "019c9a65-7d3a-7c6b-8a9d-123456789abc"
}
```

The `id` will be different for every message.

### Message fields

| Field | What it contains | Accepted value | Default |
| --- | --- | --- | --- |
| `payload` | The content to save | Text | Required |
| `priority` | The message priority | `HIGH`, `MEDIUM`, or `LOW` | Required |
| `ttl_seconds` | Seconds until the message expires | Any whole number greater than 0 | The message does not expire |

### Message errors

- `400` and `invalid_priority`: the priority is not `HIGH`, `MEDIUM`, or `LOW`.
- `400` and `invalid_ttl`: `ttl_seconds` is `0`.
- `404` and `queue_not_found`: the queue does not exist.
