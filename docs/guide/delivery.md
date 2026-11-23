# Durable lifecycle delivery

Available in 0.4.0-alpha.6. Run the ingestion worker
and API against the same reviewed runtime. Migration 011 adds private subscription
and receipt tables; [back up before migrating](recovery.md).

This REST long-poll feed delivers canonical creates and archives. It does not emit
exercise receipts, model-row changes, SSE or GraphQL subscriptions. Current caller
permissions and current source authority apply to every operation and again before
response encoding. A subscription does not grant access to a party.

## Register, process, acknowledge

Use an organization API bearer from [authentication](api.md), distinct from the
Ledger source credential. Requests below target the loopback API. Avoid putting
bearer values in shell history or publishing response files.

| Request | Body / query | Result |
| --- | --- | --- |
| `POST /v1/subscriptions` | `{"name":"my-worker","start":"retained"}` | Private `id`, signed `cursor`, expiry |
| `GET /v1/subscriptions/{id}/events` | `limit=20&waitSeconds=10`, optional `cursor` | `events`, `nextCursor`, `more` |
| `POST /v1/subscriptions/{id}/acknowledge` | `{"cursor":"issued-cursor"}` | `{"ok":true}` |
| `POST /v1/subscriptions/{id}/cancel` | `{}` | `{"ok":true}` |

`start` is `retained` or `now`. Retained means currently retained outbox records,
not complete ledger history. Now starts after the current destination head.
Retrying the same name under the same caller/grant binding reuses the subscription
without resetting progress. Changing its start mode is rejected.

Process one page in order, commit the application's effects durably, then ACK its
`nextCursor`. Use `(source generation, eventId)` as an application deduplication
key. Retried pages can repeat events, including after a server restart: delivery
is **at least once**. Exactly-once effects need an application-side transaction
covering deduplication and effects. Never ACK an event whose earlier page failed.

An omitted cursor resumes the server's durable ACK. Passing an issued cursor can
read ahead without ACKing; reconnecting without it replays unacknowledged work.
ACKs are monotonic, so a delayed older ACK cannot rewind progress. Public cursors
contain random receipt IDs rather than global outbox ordinals. They bind to the
caller, current grant policy, source plan and subscription, and expire within
60 seconds. After cursor expiry, resume without it; already ACKed work remains
ACKed. Do not edit or log cursors.

## Event shape and coverage

Responses have `formatVersion: 1`, `delivery: "at_least_once"` and coverage
`retained_authorized_outbox`. Each event contains `eventId`, `contractId`,
`kind` (`create` or `archive`), exact `template`, nullable typed `payload`,
`observedAtMicros`, `ledgerAtMicros`, `sourceOffset`, `snapshotObservation` and
`visibilityEvidence`. Integer/time/offset values are decimal strings. Raw payloads
use `{"type":"raw_json","value":"exact JSON text"}`; do not parse financial
strings into floating point. An archive has no replacement payload.

Snapshot observations are not newly submitted creations. Evidence describes a
source-witness policy or a prior-create policy without exposing parties, witness
lists, source PKs, arguments or internal batch records. Permission filtering
happens before pagination. An empty heartbeat reveals no hidden event count.

## Limits and recovery

| Limit | Value |
| --- | --- |
| Events per page | 1–25, default 20 |
| Long poll | 0–10 seconds; authority rechecked every second |
| Concurrent long polls | 4 globally, 2 per caller binding |
| Subscription lifetime | 24 hours; registration retries do not extend it |
| Subscriptions | 256 per stream, 16 per principal |
| Outstanding receipts | 64 per subscription, retained for 5 minutes |
| Recent ACK receipts | 64 |
| Worker freshness | Last ingestion observation within 30 seconds |

429 means admission/receipt backpressure; ACK committed pages, reduce concurrency
and retry with backoff. 503 means source, destination, upgrade or live-worker
readiness is unavailable; do not silently advance. Unknown or differently owned
subscriptions return 404. Changed grants or a changed source plan invalidate old
bindings: review access and register again explicitly.

Workers retain a target of 100,000 outbox events, deleting at most 256 per cycle.
Operators can request bounded pruning:

```bash
cantongrid ops prune-deliveries runtime.json --retain 100000 --limit 256
```

This never deletes canonical history. Removing an unacknowledged event visible to
this subscription makes reads fail with **409 and `coverage: "gap"`**. Hidden-only
pruning does not manufacture a gap. Retention is bounded work, not a disk quota;
monitor queue growth and consumer lag through [operations](operations.md).

A gap requires an independently verified application baseline and a new
subscription. Published alpha.6 has no atomic snapshot/feed handoff. The
[unreleased alpha.7 bootstrap API](bootstrap.md) pairs its baseline and feed
transactionally; it still reports explicit loss when retention invalidates a handoff. Reading a
snapshot then starting `now` during concurrent writes can lose changes. The
[LocalNet helper](../../examples/operations/README.md) demonstrates resynchronization
only on a controlled quiescent fixture. Do not use that sequence as a trading
consistency guarantee.
