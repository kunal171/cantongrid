# Durable delivery and bootstrap

Since 0.4.0, a REST long-poll feed delivers canonical creates and archives, at
least once.
- **What it doesn't include:** exercise receipts, model-row changes, SSE and
  GraphQL subscriptions.
- **Access:** the caller's current permissions and the current source authority
  apply to every operation. A subscription grants nothing.
- **Migrations:** 011 (subscriptions) and 012 (bootstrap). Back up before
  migrating ([operations](operations.md#backup-and-recovery)).

## Subscriptions

All requests use the organization bearer from the [API](api.md), not the source
credential.

| Request | Body / query | Returns |
| --- | --- | --- |
| `POST /v1/subscriptions` | `{"name":"my-worker","start":"retained"\|"now"}` | `id`, signed `cursor`, expiry |
| `GET /v1/subscriptions/{id}/events` | `limit=20&waitSeconds=10`, optional `cursor` | `events`, `nextCursor`, `more` |
| `POST /v1/subscriptions/{id}/acknowledge` | `{"cursor": "..."}` | `{"ok":true}` |
| `POST /v1/subscriptions/{id}/cancel` | `{}` | `{"ok":true}` |

- **Start modes:**
  - **`retained`:** starts from the outbox records still retained, not from full
    ledger history.
  - **`now`:** starts after the current head.
- **Retries:** retrying the same name reuses the subscription and its progress.
- **Order of work:** process a page in order, commit your effects, then
  acknowledge its `nextCursor`. Deduplicate on `(source generation, eventId)`,
  because retried pages repeat events.
- **Cursors:**
  - **Without a cursor:** reading resumes from the durable acknowledgement.
  - **ACKs:** they're monotonic.
  - **Binding:** each cursor is bound to the caller, grants, source plan and
    subscription.
  - **Lifetime:** cursors expire within 60 s.
  - **Handling:** never log or edit them.

**Events** carry `formatVersion: 1`, `delivery: "at_least_once"` and coverage
`retained_authorized_outbox`. Each event has these fields:
- **Identity:** `eventId`, `contractId` and `kind` (create or archive).
- **Content:** the exact `template` and a typed `payload`, using `raw_json` with
  the exact text.
- **Timing:** `observedAtMicros`, `ledgerAtMicros` and `sourceOffset`.
- **Origin:** `snapshotObservation` and `visibilityEvidence`.

Numbers are decimal strings, and no parties or witnesses are exposed.

| Limit | Value |
| --- | --- |
| Page | 1–25 events (default 20) |
| Long poll | 0–10 s, with authority rechecked every second |
| Concurrent polls | 4 globally, 2 per caller |
| Lifetime | 24 h (retries don't extend it) |
| Subscriptions | 256 per stream, 16 per principal |
| Outstanding receipts | 64 per subscription, kept for 5 min |
| Worker freshness | Last observation within 30 s |

**Errors.**
- **429:** backpressure. Acknowledge what you've done and back off.
- **503:** the source, destination or worker isn't ready. Don't advance.
- **404:** unknown or someone else's subscription.
- **Changed grants or source plan:** these invalidate the subscription, so
  register again.

**Retention.**
- **Target:** workers keep 100,000 outbox events and delete up to 256 per cycle.
- **Manual pruning:**
  `cantongrid ops prune-deliveries runtime.json --retain 100000 --limit 256`.
- **Canonical history:** never deleted.
- **Gaps:** pruning an unacknowledged visible event makes reads return
  **409 `coverage: "gap"`**. Recover with a verified baseline and a new
  subscription. Pruning only hidden events never causes a gap.

## Bootstrap

**Why a bootstrap:** taking a snapshot and then subscribing with `start: now` can
miss updates in between. A bootstrap captures a full baseline and the feed's
anchor in one transaction under the writer lock.

```http
POST /v1/subscriptions/bootstrap
{"name": "baseline-<unique id>"}
```

- **Response:** `id`, `snapshotUrl`, a signed feed `cursor`, `expiresAtSeconds`,
  `atomicHandoff: true` and coverage `observed_retained`.
- **Private values:** the source offset and outbox position stay private.
- **Snapshot paging:** page through `GET /v1/subscriptions/{id}/snapshot?limit=25`
  until `nextCursor` is null.
- **Cursor types:** snapshot cursors and feed cursors aren't interchangeable.

**Client sequence:**
1. **Register:** persist the registration ID.
2. **Stage the baseline:** stage every snapshot page privately. Concurrent changes
   wait in the feed.
3. **Publish:** publish the baseline and the subscription ID in one application
   transaction.
4. **Consume:** read the feed in order, deduplicate, apply, then acknowledge.
5. **After a restart:** resume the stored subscription instead of taking a new
   baseline.

An incomplete baseline must never replace the live view. See
`examples/bootstrap` (TypeScript).

**Retries and expiry.**
- **Same name:** retrying it returns the same pairing while it's live. It doesn't
  capture a newer baseline.
- **After the first ACK:** once post-baseline work is acknowledged, retries and
  snapshot reads return 409. Use a new name for a new baseline.
- **Expired pairings:** they return 404. Never reuse an expired or cancelled name
  in an automatic retry.

| Result | Meaning |
| --- | --- |
| 409 `coverage: gap` | Visible post-baseline events were pruned; discard staging and resync |
| 409 `coverage: unavailable` | Baseline history missing, outside retention or over budget |
| Other 409 | Name or mode conflict, or the baseline already has ACKs |
| 400 / 422 | Bad cursor or filter / bad body |
| 429 / 503 / 401 | Budget / source or worker unavailable / bad caller |

**Limits.**
- **Budget:** the bootstrap uses the [historical state](composition.md#historical-state)
  budget: 4,096 pages, 8,192 events and 64 baselines.
- **Content:** it includes contract payloads, not model rows.
- **Testing:** tests cover interleaved writes and HTTP restarts against owned
  PostgreSQL and a mock source. It hasn't been accepted on a remote network.
