# M7–M9: temporal state, durable delivery and hardening

One branch, independently testable commits. Reuse canonical contracts/events,
transactional outbox, signed caller-bound cursors and current authority checks.
No new third-party runtime dependencies are planned.

## M7: bounded temporal prototype

Expose canonical contract state at a complete source-local offset through REST
and GraphQL. Derive the observed floor from the initial snapshot and latest
selection-upgrade baseline. Never answer before that floor, during bootstrap,
ahead of a fully acknowledged boundary, or with missing/corrupt retained evidence.
The source offset is distinct from observation time and application-effective time.

Use immutable canonical payloads and create/archive evidence; do not introduce a
second event log. Recheck current caller/source rights at query and delivery.
Pagination binds to the exact requested offset. As-of means after all effects at
that offset, including same-offset create/archive pairs.

The first slice is deliberately bounded: at most 4,096 batch positions and 8,192
retained lifecycle records per stream. Exceeding the prototype budget reports
unavailable, never a confident empty state. Existing canonical tables remain
unchanged. Historical model bindings/joins/aggregates are not inferred from current
bindings; canonical temporal reads make no claim about those features.

Acceptance: snapshot floor/head, incomplete pages, missing history, exact values,
archive/recreate, same-offset effects, replay, A/B/shared scope, cursor scope and
revocation controls. Record source-local coverage explicitly.

## M8: durable authenticated subscriptions

Reuse the lifecycle outbox for a bounded long-poll feed. Register a private
subscription with an opaque random ID and a source/caller/policy binding. Start
at the current head or the retained delivery floor. Registration does not grant
source access. Each delivery and acknowledgement requires current authorities;
long polls reauthorize periodically. No source credentials or raw outbox IDs are
returned.

Signed resume positions name the last authorized event, not a global ordinal.
Persist acknowledgements so a client that loses its cursor can resume safely.
Delivery is at least once: process idempotently and acknowledge only a contiguous
processed prefix. Older acknowledgements cannot rewind progress. Backpressure,
subscriber expiry and operator-controlled bounded retention have explicit limits;
a retention gap fails explicitly instead of skipping to the current head.

Use versioned create/archive envelopes with exact JSON/string values and source
provenance. This feed is canonical lifecycle delivery, not model-row CDC. Selection
upgrades pause delivery and changed source plans require explicit resubscription.

Acceptance: reconnect, missing cursor, duplicates, out-of-order acknowledgement,
retention gaps, expiry, hidden events, authorization removal and upgrade gates.
Include owned PostgreSQL/mock-source tests and read-only retained LocalNet controls
where the existing source permits them; no new ledger writes are authorized.

## M9: hardening and operator evidence

Add authenticated operational health/metrics with no public party, payload,
credential or global progress disclosures. Record reproducible local benchmarks,
resource limits, backup/restore checks and failure/recovery/security controls.
Maintain pinned security CI and self-contained package checks.

The user confirmed there is no remote source yet. Deliver a remote setup guide
and fail-closed preflight/validation commands for certificate-verified Ledger HTTPS,
a read-only PQS tunnel, exact party grants, health and network identity. Never
invent TestNet evidence. Live remote/TestNet acceptance remains pending until an
authorized source is configured; this prevents a production-readiness claim.

Remote checks are read-only and must not upload DARs, allocate parties, change
credentials or provision paid infrastructure. Record only sanitized capabilities
and evidence, never connection secrets or raw endpoint credentials.

## M8 receipt and retention contract

Use private durable subscriptions and random receipt IDs over the existing outbox.
Never expose sequence IDs in signed JWT contents: signatures do not encrypt them.
Current permissions apply at registration, every read, acknowledgement and delivery.

Pruning must mark a subscription's gap only when it removes unacknowledged events
visible to that subscription's pinned party scope. Hidden-only pruning must not
change public heartbeats or create false gaps. Keep canonical event history intact.
Registration is idempotent by caller/scope/name. Acknowledgements advance monotonically;
old acknowledgements cannot rewind progress. Explicit signed receipt cursors support
short resumptions; missing cursors resume the durable acknowledged position.

Bound subscription lifetime, pending receipts, recent acknowledged receipts and
retention work. Canonical lifecycle feeds use typed versioned envelopes and a bounded
long-poll transport; they are not model-projection or GraphQL subscription feeds.
Revalidate during waiting and before encoding. Selection upgrades gate delivery;
changing source/policy requires an explicitly new subscription.

Random receipt IDs use PostgreSQL's built-in `gen_random_uuid`, available in the
[PostgreSQL 14 UUID functions](https://www.postgresql.org/docs/14/functions-uuid.html)
and supported PostgreSQL 17 profile; no extension or runtime dependency is needed.
