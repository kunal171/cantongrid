# Changelog

## 0.3.0-alpha.1 — unreleased

### Added

- Bounded declarative typed model definitions, filters and checked exact arithmetic.
- Deterministic generated SQL table plans and exact typed row encoding.
- Offline model validation, planning and preview CLI commands.
- Outcomia and unrelated Visibility model definitions plus owned-Postgres schema round-trip tests.

### In progress

- Phase 4 live model materialization, lifecycle retractions, model registry/backfill/promotion and generated typed APIs remain incomplete.
- These model commands do not install schemas, ingest contracts or authenticate API callers.

### Fixed

- Reject PostgreSQL-incompatible NUL text in model fields, optional values, literals and filters.
- Apply the projection-size limit to directly supplied typed rows before SQL encoding.

## 0.2.0-alpha.1 — unreleased

### Added

- Read-only, lease-checked PQS snapshot and forward-update paging with same-offset event keys.
- SQL-native canonical lifecycle materialization, durable checkpoints/outbox and observation metadata.
- Explicit raw JSON payload and prior-create archive authorization profiles with partial-history disclosure.
- Independent organization JWT authentication, reloadable scoped grants and principal/epoch-bound cursors.
- Generic authenticated REST/GraphQL, query limits and no-store responses.
- Actual existing LocalNet Visibility pipeline and owned-Postgres transport privacy regressions.

### Compatibility and limits

- Run explicit destination initialization to apply checksummed owned migration 002; migration 001 is unchanged.
- Fixture format remains 1; live runtime configuration is a separate explicit version-1 profile.
- Loopback-only LocalNet, exact template projections, raw payloads and snapshot-forward partial history.
- Actual Outcomia lifecycle and unrelated Visibility proofs passed on the explicitly authorized LocalNet; no production readiness claim.
- Generated typed models and real-time delivery remain Phase 4/5.

## 0.1.0-alpha.1 — unreleased

### Added

- Apache-2.0 project license and consistent workspace prerelease versioning.

- Rust project validation and deterministic create/archive fixture processing.
- Exact tagged values, visibility dependencies and bounded source-grant lease decisions.
- Owned local PostgreSQL materialization, atomic checkpoint/outbox and journal recovery.
- Source-context writer exclusion, migration-checksum guards and durable fixture CLI commands.
- Restricted read-only PQS schema-041 inspection and documented live-ingestion gates.
- Shared SQL authorization before rows/counts/keyset paging, with lease rechecks.
- Consolidated development commands, PostgreSQL CI and contributor/security guidance.
- Pinned security tooling for RustSec, dependency policy, redacted full-history/tree secrets and workflow linting.
- Tested SDK row compatibility and a read-only retained-window consistency probe; concurrent handoff remains unverified.

### Limits

- Fixture caller grants are simulated. Live ingestion, authenticated APIs and streams
  are incomplete; no production readiness or complete Canton history is claimed.
- The reference store has explicit batch/journal limits and loopback-only transport.
- Source archive visibility, current grants, handoff and actual Outcomia indexing
  need verified evidence before Phase 3 can pass.
