# Changelog

## 0.3.0-alpha.5 — unreleased

### In progress

- Resumable, source-authorized model rebuild jobs with bounded work and atomic activation.

## 0.3.0-alpha.4 — 2026-11-19

### Added

- Exact model equality/range/null filters and native sorting with deterministic keysets.
- Authenticated REST model discovery/query endpoints and matching GraphQL metadata/inputs.
- Generated filter/sort TypeScript interfaces and GraphQL input definitions.
- Reproducible LocalNet query controls: 54 page sequences, revision rollback,
  grant-bound cursors and fail-closed exporter recovery.
- Actual Outcomia model acceptance: 60 page sequences, private/shared markets,
  optional closure-reason promotion/rollback and 25 reconciled lifecycle events.

### Compatibility

- Model cursors now bind normalized filters and sorting; restart older model page sequences.
- No destination migration, automatic indexes or new runtime dependencies.
- Owned PostgreSQL/HTTP checks and two LocalNet query runs cover these changes;
  actual Outcomia BinaryMarket query acceptance also passes. Staged backfills and
  funded trading projections remain pending.

## 0.3.0-alpha.3 — unreleased

### Added

- crates.io package metadata, per-crate license/notice/readme files and self-contained fixture assets.
- Archive verification and a coordinated workspace publication workflow.

## 0.3.0-alpha.2 — 2026-11-18

### Added

- Exact TypeScript transport interfaces and GraphQL schema generation from model definitions.
- Authenticated generated model GraphQL at `/models/graphql`, canonical visibility before counts/pages, revision-scoped cursors and bounded operations.
- A standalone Daml prediction-market PoC, local deployment/transaction runner, privacy/recovery assertions and loopback viewer.

### Compatibility

- No new destination migration or external runtime dependency. Dynamic schema support uses the existing approved async-graphql dependency.
- The PoC and generated model API are available in 0.3.0-alpha.2; they were not included in 0.3.0-alpha.1.

## 0.3.0-alpha.1 — 2026-11-18

### Added

- Bounded declarative typed model definitions, filters and checked exact arithmetic.
- Deterministic generated SQL table plans and exact typed row encoding.
- Offline model validation, planning and preview CLI commands.
- Owned model revision registry with atomic/idempotent registration, integrity checks
  and operator `models register` / `models inspect` commands.
- Atomic `models install` for private empty projection tables, with actual schema
  inspection, safe retries and rejection of unmanaged tables or drift.
- Source-scoped `models bind`, bounded atomic backfill/promotion and transactional
  live creation/archive projections, with schema/lease checks and rollback controls.
- Outcomia and unrelated Visibility model definitions plus owned-Postgres schema round-trip tests.
- GitBook-ready developer guide covering setup, persistence, LocalNet, authentication,
  REST/GraphQL, model examples and operations, with synthetic preview payloads.

### In progress

- Phase 4 generated typed APIs, revision-bound cursors and actual LocalNet model acceptance remain incomplete.
- Installation does not ingest contracts, activate live processing or authenticate API callers.

### Fixed

- Consolidate developer/reference docs, remove obsolete planning and comparison pages, and correct stale status claims.
- Reject PostgreSQL-incompatible NUL text in model fields, optional values, literals and filters.
- Apply the projection-size limit to directly supplied typed rows before SQL encoding.

### Compatibility

- Run explicit `storage init` to apply owned migrations through 005 before opening
  the destination. Migrations 001–004 are unchanged. Older binaries reject unknown
  migration sets; downgrade requires a separate reviewed plan.

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
