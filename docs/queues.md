# Queues and messages

Retsu can create queues, add messages, and return the next waiting message.
Completing or retrying messages is not available yet.

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

The visibility timeout is recorded when a message is returned. The delivery
attempt limit is saved for retry support, which is not available yet.

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

## Get the next message

Use the queue name in the address. The request does not need a body:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues/emails/messages/dequeue
```

A successful request returns status `200` and the next message:

```json
{
  "id": "019c9a65-7d3a-7c6b-8a9d-123456789abc",
  "payload": "send welcome email",
  "priority": "HIGH",
  "receipt_handle": "d03de2b6-22d6-46f5-a662-3af5d46f7054",
  "delivery_attempts": 1
}
```

Retsu returns the highest-priority waiting message. Messages with the same
priority are returned in the order they were added. Expired messages are
skipped.

The returned message is no longer waiting for another request. Completing or
retrying it is not available yet.

### Response fields

| Field | What it means |
| --- | --- |
| `id` | The message ID |
| `payload` | The saved message content |
| `priority` | `HIGH`, `MEDIUM`, or `LOW` |
| `receipt_handle` | Identifies this delivery |
| `delivery_attempts` | Times the message has been returned |

If no message is waiting, Retsu returns status `204` with no response body.

If the queue does not exist, Retsu returns `404` and `queue_not_found`.
