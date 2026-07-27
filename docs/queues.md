# Queues and messages

Retsu can create queues, add messages, return the next waiting message, and
mark a returned message as complete.

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

The visibility timeout controls how long a returned message can be completed.
The worker makes timed-out messages available again until the delivery attempt
limit is reached. Messages that reach the limit are stored separately.

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

The returned message is not available to another request during the queue's
visibility timeout.

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

## Complete a message

Use the `id` and `receipt_handle` from the latest response:

```bash
curl --request POST \
  http://127.0.0.1:2424/v1/queues/emails/messages/019c9a65-7d3a-7c6b-8a9d-123456789abc/acknowledge \
  --header 'content-type: application/json' \
  --data '{
    "receipt_handle": "d03de2b6-22d6-46f5-a662-3af5d46f7054"
  }'
```

A successful request returns status `204` with no response body. The message is
removed and will not be returned again.

### Completion errors

- `404` and `queue_not_found`: the queue does not exist.
- `404` and `message_not_found`: the message does not exist in this queue.
- `409` and `invalid_receipt_handle`: the receipt handle is not from the
  message's current delivery, or its visibility timeout has passed.

## Try a timed-out message again

Run the worker alongside the API:

```bash
just worker queue visibility-timeout-processor
```

If a returned message is not completed before its visibility timeout, the
worker makes it available again while its `delivery_attempts` value is below
the queue's `max_delivery_attempts` setting. A later request can return the
message with a new `receipt_handle` and a higher `delivery_attempts` value. The
old receipt handle will no longer work.

When a message reaches the delivery attempt limit without being completed,
Retsu removes it from the active queue and stores it separately. There is no
API to view or restore these messages yet.
