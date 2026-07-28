# Caching

Retsu has two independent cache layers:

- a process-local in-memory cache for `queue_id -> queue_name`;
- a distributed Redis-protocol cache for complete queue details.

PostgreSQL remains the source of truth. Dragonfly is used by the local
development stack, but the application uses Redis-protocol names and commands
so Redis or Valkey can replace it.

## Read paths

A queue-name lookup checks the in-memory cache first. A miss uses the normal
queue-details path, extracts the name, and stores only that name in memory.

```text
queue_name(queue_id)
    -> in-memory queue-name cache
    -> distributed queue-details cache
    -> PostgreSQL
```

A queue-details lookup deliberately skips the process-local name cache:

```text
queue_details(queue_id)
    -> distributed queue-details cache
    -> PostgreSQL
```

The in-memory cache is private to one process. The distributed cache is shared
by API and worker replicas.

## Write-through

Queue creation writes in this order:

```text
PostgreSQL
    -> distributed queue-details cache
    -> in-memory queue-name cache
```

Cache writes happen only after PostgreSQL accepts the queue. Cache failures are
logged and do not turn a committed database write into an API failure.

Queue mutations use write-through replacement rather than cache invalidation.
An update persists PostgreSQL first, then overwrites L2 and L1 as the repository
call returns through the decorators.

## Stampede protection

Moka coalesces concurrent in-memory misses for the same queue ID within one
process.

On a distributed miss, a process claims a short-lived per-queue load lock using
Redis `SET NX PX`. The lock holder checks the distributed value again, loads
PostgreSQL once, and populates the distributed cache. Other processes wait for
the value or for the lock lease to expire before attempting to load.

The lock expires after its short lease; no explicit unlock command is needed.
If the distributed cache is unavailable, Retsu fails open to PostgreSQL;
per-process Moka coalescing still applies.

## Expiration

Complete queue details have a five-minute safety TTL.

Missing queues are cached for five seconds. This bounds repeated database reads
for nonexistent queue IDs while allowing a later creation to become visible
quickly.

Queue names have no time-based expiration because names are immutable. They can
still be evicted when their process-local cache reaches either configured
capacity limit.

## Configuration

```yaml
cache:
  in_memory:
    enabled: true
    regions:
      queue_names:
        max_entries: 10000
        max_capacity_bytes: 8388608
  distributed:
    enabled: true
    url: redis://127.0.0.1:24251
    connection_timeout_milliseconds: 50
    command_timeout_milliseconds: 20
```

Environment overrides follow the same hierarchy:

```bash
RETSU_CACHE__IN_MEMORY__ENABLED=false
RETSU_CACHE__IN_MEMORY__REGIONS__QUEUE_NAMES__MAX_ENTRIES=20000
RETSU_CACHE__DISTRIBUTED__ENABLED=false
RETSU_CACHE__DISTRIBUTED__URL=redis://cache.internal:6379
```

The distributed connection is multiplexed and reconnecting. A connection
attempt has a 50 ms default budget and each command has a 20 ms default budget.

The two cache layers can be enabled independently. The decorator remains in the
repository chain when disabled, but its cache client is not constructed and it
delegates directly to the next repository:

- no in-memory cache makes `queue_name` use `queue_details` directly;
- no distributed cache makes `queue_details` use PostgreSQL directly;
- disabling both makes all queue metadata reads use PostgreSQL.

## Metrics

Retsu exports:

| Metric | Labels | Meaning |
| --- | --- | --- |
| `cache.requests` | `cache.name`, `outcome` | Cache hits and misses |
| `cache.load.duration` | `cache.name`, `outcome` | Source-load latency after a miss |

The cache names are `queue_names` and `queue_details`. Load outcomes are
`success`, `not_found`, and `error`.

## Main implementation files

- `src/cache/memory.rs` contains the process-local Moka cache.
- `src/cache/redis_protocol.rs` contains the vendor-neutral Redis-protocol
  connection and commands.
- `src/modules/queue/infrastructure/l2.rs` owns queue-details keys,
  serialization, TTLs, distributed load locking, and PostgreSQL fallback.
- `src/modules/queue/infrastructure/l1.rs` owns process-local queue-name
  caching and delegates misses to the distributed repository.
