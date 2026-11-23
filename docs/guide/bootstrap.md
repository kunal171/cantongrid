# Bootstrap a baseline and durable feed

Available in the **unreleased alpha.7 source checkout**. Build matching worker/API
binaries and explicitly apply owned migration 012 after backing up. Migrations
001–011 remain unchanged. Older binaries reject the new installation; rollback
requires the pre-upgrade database and matching configuration/keys.

Ordinary snapshot-then-`start: now` can miss updates between the two operations.
The bootstrap endpoint captures a complete canonical baseline and its durable feed
anchor in one transaction under the ingestion writer lock. New events are queued
after that anchor while the client loads the fixed baseline.

## Requests

Use the organization bearer described in [API authentication](api.md), distinct
from the Ledger source credential. No request supplies parties or source offsets.

```http
POST /v1/subscriptions/bootstrap
Content-Type: application/json
Authorization: Bearer <organization bearer>

{"name":"baseline-<new-unique-client-id>"}
```

The response contains `id`, `snapshotUrl`, a signed delivery `cursor`,
`expiresAtSeconds`, `formatVersion: 1`, `atomicHandoff: true` and coverage
`observed_retained`. The captured source offset and outbox position remain private;
JWT signatures do not encrypt claims, so neither is embedded in a public cursor.

Page `GET /v1/subscriptions/{id}/snapshot?limit=25`, passing its `nextCursor` on
subsequent pages. Results contain `items`, exact string `visibleTotal`,
`nextCursor`, format version and pinned-baseline provenance. A null cursor ends
the baseline. Limits are 1–25, default 20. Snapshot cursors cannot be used as feed
cursors or on another subscription. Current caller/source grants apply each time.

After committing the complete baseline locally, use the existing
[delivery API](delivery.md): `GET /v1/subscriptions/{id}/events`, then ACK only
pages whose application effects committed. Omit a feed cursor to resume the durable
ACK; the registration cursor is optional and short-lived.

## Client sequence

1. Use a fresh unique name and persist the in-progress registration identity.
2. Stage every snapshot page in private application storage. Concurrent creates
   and archives do not change that baseline; their effects wait in the feed.
3. Publish the staged view and subscription identity in one application transaction.
4. Consume events in order. Atomically deduplicate by the configured source
   generation and `eventId`, apply effects, then ACK the processed prefix.
5. After restart, resume the stored subscription instead of replacing the baseline.

An incomplete baseline must never replace the live application view. A lost ACK
response can replay a page, so application effects must be idempotent. This API
provides a consistent handoff and at-least-once delivery, not exactly-once effects
in another database. See the [TypeScript example](../../examples/bootstrap/README.md).

## Retries, expiry and failures

Same-name retries reuse the original pairing while it is live and unconsumed;
they do not capture a newer baseline or extend the 24-hour lifetime. Ordinary
subscriptions and bootstrap names cannot be interchanged. Once post-baseline
work is ACKed, registration retries and snapshot reads return 409: use a new name
for an explicit new baseline. Continuing the existing feed remains valid.

A retained expired pairing returns 404. Cleanup/cancellation eventually removes
names, so idempotency is bounded by the subscription lifetime. Keep request retry
deadlines shorter than that lifetime and never reuse cancelled/expired names for
an automatic retry. Use the persisted ID for recovery and a new name for a new
baseline. Cursor expiry alone does not erase durable progress; restart baseline
paging on the same live pairing if its page cursor expires.

| Result | Meaning |
| --- | --- |
| 409, `coverage: gap` | Visible post-baseline events were pruned; discard incomplete staging and explicitly resynchronize |
| 409, `coverage: unavailable` | Baseline history is missing, outside the retained floor, over the temporal budget, or not at a complete boundary |
| Other 409 | Name/mode conflict or the baseline already has post-baseline ACKs |
| 404 | Unknown, differently owned or expired subscription |
| 400 / 422 | Invalid filter/cursor / malformed registration body |
| 429 | Shared subscription or request budget exceeded |
| 503 | Current source/health unavailable, stale worker during registration, writer busy or upgrade gated |
| 401 | Organization bearer or current caller grant invalid |

Hidden-only updates and hidden-only pruning do not change the visible snapshot,
advance its public cursor or manufacture a retention gap. Snapshot pages omit
source checkpoints, floors and global counters. Current permission changes
invalidate bindings; do not combine pages from different scopes.

This uses the bounded [canonical temporal prototype](temporal.md): 4,096 journal
pages, 8,192 events and 64 package baselines. A snapshot includes canonical contract
payloads, not historical model bindings or model CDC. New registrations require a
fresh ingestion observation and a fully completed source interval. Existing pinned
snapshot reads can continue under current authority while the worker is stale;
live feed delivery waits for worker readiness. Retention and package-baseline
changes can invalidate a handoff explicitly; they cannot silently move its anchor.

Owned-PostgreSQL/mock-source tests exercise interleaved writes and HTTP restart.
The retained LocalNet helper performs no ledger writes; its real-source controls
are distinct from simulated concurrent lifecycle writes. Remote/TestNet and
production acceptance remain pending authorized access.
