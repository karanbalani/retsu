# Queues and messages

Retsu can create and configure queues, add messages, return the next waiting
message, and mark a returned message as complete.

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
  "max_delivery_attempts": 5,
  "default_message_ttl_seconds": 604800
}
```

The `id` will be different for every queue. Keep it for message operations, which identify the queue by this stable ID rather than by its name.

### Queue settings

The visibility timeout controls how long a returned message can be completed.
After that time, dequeue can claim the message again until the delivery attempt
limit is reached. A later dequeue stores messages that reach the limit
separately. The default message lifetime controls when messages expire.

| Field | What it controls | Accepted value | Default |
| --- | --- | --- | --- |
| `name` | How the queue is identified | 1–64 lowercase letters, numbers, dots, underscores, or hyphens | Required |
| `visibility_timeout_seconds` | Seconds before an unfinished message can be tried again | 1–21,600 seconds | 30 |
| `max_delivery_attempts` | Times a message can be tried | 1–100 | 5 |
| `default_message_ttl_seconds` | Seconds before a message expires | 1–2,592,000 seconds | 604,800 (7 days) |

A name must start and end with a letter or number.

To change the optional settings:

```bash
curl --request POST http://127.0.0.1:2424/v1/queues \
  --header 'content-type: application/json' \
  --data '{
    "name": "emails",
    "visibility_timeout_seconds": 60,
    "max_delivery_attempts": 10,
    "default_message_ttl_seconds": 86400
  }'
```

### Queue errors

- `400` and `invalid_queue_name`: the name does not follow the rules above.
- `400` and `invalid_visibility_timeout`: the value is outside the allowed
  range.
- `400` and `invalid_max_delivery_attempts`: the value is outside the allowed
  range.
- `400` and `invalid_default_message_ttl`: the value is outside the allowed
  range.
- `409` and `queue_already_exists`: another queue already uses the same name.

## Update queue configuration

Use `PATCH` with the stable queue ID. Only the fields included in the request
change; queue names are immutable.

```bash
curl --request PATCH \
  http://127.0.0.1:2424/v1/queues/019c9a65-7d3a-7c6b-8a9d-123456789abc \
  --header 'content-type: application/json' \
  --data '{
    "visibility_timeout_seconds": 60,
    "max_delivery_attempts": 10
  }'
```

A successful request returns status `200` and the complete updated queue:

```json
{
  "id": "019c9a65-7d3a-7c6b-8a9d-123456789abc",
  "name": "emails",
  "visibility_timeout_seconds": 60,
  "max_delivery_attempts": 10,
  "default_message_ttl_seconds": 604800
}
```

The update is written to PostgreSQL first and then replaces the distributed
queue-details entry and process-local queue-name entry.

### Update errors

- `400` and `empty_queue_update`: no configuration field was provided.
- `400` and the relevant validation code from the queue settings table: a
  supplied value is outside its accepted range.
- `404` and `queue_not_found`: the queue does not exist.

## Add a message

Use the queue ID returned when the queue was created:

```bash
curl --request POST \
  http://127.0.0.1:2424/v1/queues/019c9a65-7d3a-7c6b-8a9d-123456789abc/messages \
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
  "id": "019c9a66-2d13-7be7-9728-123456789abc"
}
```

The `id` will be different for every message.

### Message fields

| Field | What it contains | Accepted value | Default |
| --- | --- | --- | --- |
| `payload` | The content to save | Text | Required |
| `priority` | The message priority | `HIGH`, `MEDIUM`, or `LOW` | Required |
| `ttl_seconds` | Seconds until the message expires | 1–2,592,000 seconds | The queue's `default_message_ttl_seconds` |

Retsu resolves the effective TTL from the message or the distributed
queue-details cache before writing the message. The insert uses PostgreSQL's
current timestamp to calculate the expiry and does not read the `queue` table.

### Message errors

- `400` and `invalid_path`: the queue ID is not a valid UUID.
- `400` and `invalid_priority`: the priority is not `HIGH`, `MEDIUM`, or `LOW`.
- `400` and `invalid_ttl`: `ttl_seconds` is outside the accepted range.
- `404` and `queue_not_found`: the queue does not exist.

## Get the next message

Use the queue ID in the address. The request does not need a body:

```bash
curl --request POST \
  http://127.0.0.1:2424/v1/queues/019c9a65-7d3a-7c6b-8a9d-123456789abc/messages/dequeue
```

A successful request returns status `200` and the next message:

```json
{
  "id": "019c9a66-2d13-7be7-9728-123456789abc",
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
  http://127.0.0.1:2424/v1/queues/019c9a65-7d3a-7c6b-8a9d-123456789abc/messages/019c9a66-2d13-7be7-9728-123456789abc/acknowledge \
  --header 'content-type: application/json' \
  --data '{
    "receipt_handle": "d03de2b6-22d6-46f5-a662-3af5d46f7054"
  }'
```

A successful request returns status `204` with no response body. If the receipt
handle identifies the message's current unexpired delivery, the message is
removed and will not be returned again.

Acknowledgement is idempotent. Retsu also returns `204` when the message has
already been removed, the receipt handle is stale, or its visibility timeout
has passed. A stale receipt handle never removes a newer delivery of the
message.

### Completion errors

- `404` and `queue_not_found`: the queue does not exist.

## Try a timed-out message again

If a returned message is not completed before its visibility timeout, the
next dequeue request can claim it directly while its `delivery_attempts` value
is below the queue's `max_delivery_attempts` setting. The returned message has
a new `receipt_handle` and a higher `delivery_attempts` value. Acknowledging
with the old receipt handle has no effect.

When a message reaches the delivery attempt limit without being completed,
the next dequeue request removes it from the active queue and stores it
separately before looking for another deliverable message. There is no API to
view or restore these messages yet.

## Remove expired messages

Messages use their own `ttl_seconds` value when provided. Otherwise, they use
the queue's `default_message_ttl_seconds` setting. Retsu stops returning a
message as soon as its lifetime ends.

Run the expired message cleaner as a separate process:

```bash
just worker queue expired-message-cleaner
```

The cleaner permanently removes expired waiting messages. If a returned message
expires, the cleaner waits for its visibility timeout to end before removing
it.
