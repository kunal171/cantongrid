# Atomic canonical snapshot/feed handoff

Build on alpha.6's immutable canonical history, complete source boundaries and
private durable subscriptions. No new runtime dependencies or ledger writes.
This feature does not introduce model CDC or historical model bindings.

## Contract

One authorized destination transaction must capture a proven complete boundary B
and persist a caller/source/policy-bound subscription anchored immediately after
all lifecycle effects through B. The per-stream writer lock serializes this with
ingestion, selection upgrades and retention. Caller/source authority is checked
before committing and again before returning a signed response.

The response supplies the exact string B and the private subscription. Clients
page canonical as-of B using the existing API, stage the baseline locally, then
consume the subscription. Every visible event after B must be available for
replay or produce an explicit retention gap. Baseline paging never advances ACKs.
Clients commit baseline/effects and deduplication durably before acknowledging.

Reject creation during initial bootstrap, incomplete forward intervals, stale
worker observations, upgrade gates, missing coverage or temporal work-budget
exhaustion. Do not guess a boundary from timestamps, PKs or exporter readiness.

## Persistence and retries

Use a new explicit migration; leave 001–011 untouched. Persist the baseline offset
and original delivery anchor alongside the subscription. Same-name retries must
return the original pairing, never silently capture a newer boundary. Distinguish
ordinary subscriptions from bootstrap subscriptions. Once post-baseline work has
been ACKed, do not offer the original baseline as a fresh reset; require a new
client name for a new baseline. Current grants and source-plan bindings apply to
all retries, snapshot pages, reads and ACKs.

A baseline can outlive short signed cursors; reconnect using durable state and
fresh signed cursors. Changed grants, package baselines, pruning, receipt expiry
and subscription expiry must fail or resume explicitly. Never claim success when
pruning has removed visible post-baseline events.

## Small implementation steps

1. Document the contract; test an exact complete-boundary predicate independently.
2. Add private bootstrap metadata and storage registration under the writer lock.
3. Add authenticated REST bootstrap with stable retry/error behavior.
4. Test concurrent ingest during baseline paging, hidden events, archives/recreates,
   restart/ACK, retention, expiry, revocation, selection upgrades and bounds.
5. Add a consumer example and an owned-database acceptance script. LocalNet reuse
   stays read-only. Keep remote/TestNet acceptance pending configured access.

Do not advertise atomic handoff until storage/API concurrency controls pass.
