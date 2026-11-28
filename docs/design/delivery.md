# Design: history, delivery and hardening

The contracts behind M7 (temporal reads), M8 (durable delivery), the 0.4.0 atomic
bootstrap and M9 (hardening).
- **Usage:** [composition](../guide/composition.md#historical-state),
  [delivery](../guide/delivery.md) and [operations](../guide/operations.md).
- **Reuse:** they rely on canonical contracts and events, the transactional
  outbox, signed caller-bound cursors and current authority checks.
- **Additions:** no new runtime dependency, and no second event log.

## M7: bounded temporal reads

- **What it answers:** canonical membership at a complete source-local offset,
  meaning after every effect at that offset, including create and archive pairs at
  the same offset.
  - **Floor:** the initial snapshot or the latest selection-upgrade baseline.
  - **Never answered:** below the floor, while bootstrapping, ahead of an
    acknowledged boundary, or when evidence is missing.
- **Time:** the offset is neither the observation time nor the application's
  effective time.
- **Budget:** 4,096 batch positions and 8,192 lifecycle records per stream.
  Exceeding it gives unavailable, never an empty result.
- **Not inferred:** historical model bindings, joins and aggregates.
- **Acceptance:** the floor and head, incomplete pages, missing history,
  archive and recreate, effects at the same offset, replay, A/B/shared scope, and
  cursor revocation.

## M8: durable subscriptions

- **Registration:**
  - **Subscription:** private, with a random ID and bound to the source, the
    caller and the policy.
  - **Idempotent:** by caller, scope and name.
  - **No access granted:** registering doesn't grant source access.
- **Authorization:** current authority applies at registration, on every read,
  on every ACK and at delivery. Long polls reauthorize while they wait.
- **Cursors:**
  - **Contents:** they name the last authorized event through a random receipt ID
    (`gen_random_uuid`).
  - **No outbox sequence numbers:** a JWT signature doesn't encrypt its contents,
    so sequence numbers aren't put in cursors.
  - **Durable ACKs:** acknowledgements are persisted and monotonic, so a client
    can resume without a cursor.
- **Delivery:** at least once, so clients acknowledge a contiguous processed
  prefix.
- **Gaps:**
  - **When:** a gap is marked only when pruning removes unacknowledged events
    visible to that subscription's party scope.
  - **Hidden pruning:** pruning only hidden events changes nothing.
  - **History:** canonical history is kept.
- **Bounded resources:** lifetime, pending receipts, recent receipts and retention
  work.
- **Gates:** selection upgrades pause delivery, and a changed source plan needs a
  new subscription.
- **Envelopes:** create and archive envelopes are versioned, with exact values.
  They aren't model-row CDC.

## Atomic bootstrap

- **One transaction:**
  - **What it captures:** a proven complete boundary B and a subscription anchored
    just after B.
  - **Locking:** under the per-stream writer lock, so it serializes with
    ingestion, upgrades and retention.
  - **Authorization:** checked before the commit and again before the response.
- **B stays private:** exposing it would reveal hidden activity.
  - **Instead:** clients get a subscription and a snapshot URL bound to it.
  - **No ACKs from paging:** paging the baseline never advances ACKs.
- **Refused:** during the initial bootstrap, with incomplete intervals, a stale
  worker, an upgrade gate, missing coverage or an exhausted budget. A boundary is
  never guessed from timestamps or keys.
- **Migration 012:** stores the private baseline offset and anchor on the
  subscription.
  - **Retries:** a same-name retry returns the original pairing.
  - **Name rules:** bootstrap names can't be reused by ordinary subscriptions.
  - **After an ACK:** once post-baseline work is acknowledged, the baseline can't
    be restarted, so use a new name.
- **Failures:** visible retention loss returns `coverage: gap`, and missing
  temporal coverage returns `coverage: unavailable`.

## M9: hardening

- **Health and metrics:** authenticated, with no parties, payloads, credentials or
  global progress.
- **Evidence:** reproducible local benchmarks, resource limits, backup and restore
  checks, and security CI with pinned versions.
- **Remote setup:** a guide plus fail-closed checks for verified Ledger HTTPS, a
  read-only PQS tunnel, exact grants, health and network identity.
  - **Read-only:** no DAR uploads, parties, credentials or paid infrastructure.
  - **Acceptance:** remote acceptance waits for an authorized source, so no
    TestNet evidence is claimed.
