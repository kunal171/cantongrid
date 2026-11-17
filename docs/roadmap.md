# Phased delivery

Phases 0 and 2 are complete at their documented scope. The Phase 1 LocalNet/PQS
baseline and visibility experiments are recorded in [LocalNet evidence](localnet-evidence.md);
remaining live connector gates continue to apply. Phase 2 acceptance is recorded in
[verification](verification.md) and the [fixture workflow](fixtures.md). This does
not mark every Phase 1 capability verified. Later acceptance items below are
requirements, not statements that their checks have passed.

## Phase 0 — Documentation foundation

Deliver the product contract, architecture, developer interface, privacy model,
planned CLI, research gates and verification plan. Distinguish accepted decisions
from unresolved protocol details. No runtime scaffolding or live source claims.

## Phase 1 — Compatibility and LocalNet research

Inspect the available LocalNet setup and official supported interfaces. Record
versions, source authorization, snapshot/update handoff, lifecycle visibility,
history limits and verified package metadata. Select the first connector only
when evidence supports it. PQS is the preferred candidate, not an assumed API.

Inspect actual Outcomia Daml packages and select a small unrelated fixture.
Define exact type mappings and access/revocation behavior. Acceptance is a
reproducible, credential-free compatibility report and an implementation plan
with resolved gates from [research decisions](research-decisions.md).

## Phase 2 — Core configuration and fixture pipeline (complete)

Build the Rust workspace, CLI validation, source-neutral identities, capability
contracts, policy handling and deterministic fixture ingestion. Use explicit
model/schema versions and exact values from the start. Establish meaningful
unit and integration checks, including replay and A/B/AB isolation.

Acceptance: documented fixture commands reproduce lifecycle state without source
credentials. The engine contains no Outcomia-specific branches. Fixture success
is labelled fixture success and does not imply LocalNet compatibility.

Delivered: `init`, `validate` and `fixture run`; strict JSON v1; source/network/scope
identities; declared capabilities; exact tagged values; atomic in-memory batch
application; conflict/dedup detection; scoped reads with bounded leases; and two
examples through one runner. Fixture caller grants are simulated and state is
in memory. Models, live adapters and other planned CLI commands fail explicitly.
Advanced Daml types/schema discovery remain restricted pending verification.

## Phase 3 — Authorized LocalNet ingestion and baseline queries

Implement the verified read-only connector, owned PostgreSQL canonical storage,
atomic checkpoints/outbox, snapshot/update handoff and restart recovery. Add
authenticated generic active-contract and lifecycle REST/GraphQL queries with
coverage metadata. Demonstrate Outcomia and the unrelated example through the
same selection interface.

Acceptance: LocalNet evidence covers initial snapshot, live changes, duplicates,
restart, authorization changes and incomplete-history behavior. Verify exact
numeric round trips and A/B/AB isolation through both API transports.

## Phase 4 — Declarative processing and generated models

Introduce the bounded processing vocabulary incrementally: typed projections,
filters and exact calculations first; grouping, aggregates and joins only with
explicit lifecycle and privacy semantics. Generate managed storage, migrations,
query fields and versioned rebuilds from model definitions.

Acceptance: both examples add useful derived models without handwritten engine
changes, SQL migrations or resolvers. Replay produces equivalent results;
breaking changes preserve the previous usable version until controlled promotion.
Test authorization dependencies and archival retraction for each operator.

## Phase 5 — Durable streams and first release readiness

Deliver resumable authenticated SSE from the durable outbox, retention/gap
behavior, stream reauthorization, operational diagnostics and the local `dev`
workflow. Document installation and reproducible demos. Add capacity measurements
and practical limits; do not invent throughput claims. Review licenses and
publish an actual license before public distribution.

Acceptance: crash/reconnect and cursor-expiry tests pass, no private data leaks
across principals, and end-to-end LocalNet demos work for both applications.
Real devnet validation remains a separate gate when infrastructure is available.
Public Scan and other tracks remain deferred.

## Small-commit working agreement

Keep each commit reviewable around one concern: configuration, an adapter,
a migration, an API slice or a processing operator. Include relevant docs and
verification with that concern. Avoid combining repository scaffolding, source
integration and complete API generation in one commit.

At each phase, report what works, the environment actually tested, remaining
gates and commit references. Finish the relevant checks before advancing;
never substitute a successful fixture run for live integration evidence.
