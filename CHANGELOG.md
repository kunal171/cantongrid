# Changelog

## 0.5.0-alpha.2 — 2026-11-23 (unreleased)

- Begin authenticated reconciliation APIs with operator-pinned policies and
  current source/caller checks. No new dependency or migration is planned.

## 0.5.0-alpha.1 — 2026-11-23

- Add bounded versioned exact-value reconciliation policies with canonical
  fingerprints, checked numeric comparisons and sanitized fixture reports.
- Add offline `reconcile validate` / `reconcile evaluate` and synthetic backing /
  supply examples. Supplied fixtures do not prove authorization or source
  completeness or a financial all-clear.
- Add offline `reconcile preflight` to validate policy references against an exact
  projected model definition; bind identities without checking source or active state.
- Add storage-only indexed per-contract reconciliation with complete source/caller
  scope, coherent active-model snapshots, retained coverage, projection checks
  and bounded redacted summaries. No public route, migration or new dependency.

## 0.4.0 — 2026-11-23

- Promote the verified node-local contracts profile from alpha.7 with unchanged
  runtime implementation and migrations 001–012.
- Include scoped composition/inspection, durable lifecycle delivery and canonical
  bootstrap with each feature's verification level documented.
- Canonical temporal reconstruction remains a bounded experimental prototype.
  Remote/TestNet and production acceptance remain pending authorized access.
- From 0.3.0, back up before explicitly applying owned migrations through 012.
  Existing alpha.7 installations need no migration or automatic reindex.

## 0.4.0-alpha.7 — 2026-11-23

- Add authenticated atomic canonical bootstrap: pair a fixed baseline and durable
  feed under one writer lock, with opaque snapshot URLs and current-grant checks.
- Migration 012 privately stores the baseline offset/anchor; retries cannot move
  the baseline or reset acknowledged effects. Retention loss fails explicitly.
- Share strict schema041 position validation between temporal coverage and package
  upgrade boundaries, rejecting duplicate/unknown fields and incomplete ties.

## 0.4.0-alpha.6 — 2026-11-23

- M7 bounded canonical state at an acknowledged source offset, with current
  authorization, exact values and explicit unavailable-history responses.
- M8 canonical lifecycle long polling with private subscriptions, durable monotonic
  ACKs, opaque receipt cursors, bounded backpressure and visible-only retention gaps.
  Migration 011 is explicit; canonical history remains retained.
- M9 worker freshness/readiness, operator pressure reports, offline profile checks,
  read-only source preflight, isolated exact backup/restore and local benchmarks.
- Read-only-ledger LocalNet temporal/replay/recovery controls and stronger local TLS
  credential-forwarding tests. Remote/TestNet validation is pending an authorized
  source; setup instructions are included. No production capacity claim.

## 0.4.0-alpha.5 — 2026-11-22

- Read-only authorized transaction timelines and native per-contract projection
  diagnostics, with sanitized local evaluation/progress evidence commands.
- Visible-only ordinals and counts; no raw arguments, witness lists, payloads,
  source-node gaps or global progress exposed to API callers.
- Existing destination migrations, source plans and model fingerprints unchanged.
- M6 offline sync impact/dependency reports with exact generated table/API identities,
  explicit rebuild requirements and relationship revision-review blockers.
- `sync summary`, sanitized `sync evaluate` and bundled `impact.txt`; incompatible
  plans retain their report and cannot generate an applicable bundle.

- Update the existing `url` dependency to 2.5.9 (and `form_urlencoded` to
  1.2.3) for RUSTSEC-2026-0592.

## 0.4.0-alpha.4 — 2026-11-22

- Opt-in typed creation provenance and conservative direct exercise-to-create
  proofs, with private atomic edge/outbox storage in migration 010.
- Authenticated one-hop REST/GraphQL lineage queries with independent endpoint
  visibility, full proof dependencies and root/direction-bound cursors.
- Generic and existing Outcomia LocalNet acceptance; unknown/filtered subtrees
  remain unproven. Lineage stays separate from declared links and joins; no
  transitive traversal.

- M3 versioned declared relationships over exact projected keys, generated client
  interfaces and guarded current-state REST/GraphQL queries. Declared links remain
  distinct from source-proven lineage; no joined payloads or traversal.

- M4 opt-in typed inner joins with independently visible inputs, selected values,
  exact generated clients and visible-result cursor invalidation. No outer joins,
  hidden-input placeholders or changes to aggregate authorization.

## 0.4.0-alpha.3 — 2026-11-21

- Opt-in schema041 tree-mode exercise receipts, bounded complete forward
  transactions and atomic private receipt journal/outbox (destination migration
  009). Existing contract APIs retain create/archive behavior; no graph API.
- Generic Daml exercise fixture, scoped source comparison and real LocalNet
  worker controls. Package handoff for receipt streams remains unsupported.

## 0.4.0-alpha.2 — 2026-11-21

- Offline LF 2.1 DAR metadata inspection with verified payload hashes, qualified
  type/contract signatures and bounded archive traversal. New `cantongrid-metadata`
  crate and `metadata inspect` command; no Ledger or database writes.
- Bounded compound row sorting with exact tuple keysets, compatible REST objects,
  GraphQL `orderBy` and generated client/schema updates (#21).
- Reviewed additive package upgrades with private durable snapshot pages, gated
  publication/cancellation, immutable source checkpoints and model rollback (#25).
  Explicit destination migrations 007–008; 001–006 are unchanged.
- Verified remote HTTPS transport policy for Ledger and health endpoints, with
  certificate verification, redirect refusal, connection timeouts, and identity-provider/issuer
  checking (#22).
- Offline command-driven contract schema synchronization and review bundle generation
  from verified DAR signatures and explicit model selections. New `cantongrid-sync`
  crate and `sync plan` / `sync bundle` commands (#24).

## 0.4.0-alpha.1 — 2026-11-20

- Declarative grouped counts and exact sums over maintained model contributions,
  with generated REST/GraphQL/TypeScript and separate signed cursor domains.
- Data-independent full-source aggregate access; whole-group witness checks and
  explicit all-or-nothing model GraphQL failures. Existing row queries keep their scopes.
- Outcomia/Visibility examples and read-only LocalNet comparison with a Ledger snapshot.
- Optional aggregate declarations preserve existing model fingerprints; no new migration
  or runtime dependency. Totals are calculated at read time, not separately materialized.

## 0.3.0 — 2026-11-20

- Regular release of the verified LocalNet contracts/indexing/model API profile.
- Digest-pinned PostgreSQL 17.12 source and destination fixtures in separate new
  volumes. Existing 14.x data requires a logical restore; original volumes remain.
- Real 261-contract staged rebuild acceptance: process restart, source revocation,
  concurrent creates/archive, stale-activation rejection, promotion and rollback.
- Reproducible write-enabled PoC command, private journals and sanitized evidence.
- Runtime APIs and migrations 001–006 are unchanged from alpha.5. Funded trading,
  joins/aggregates, streams and production acceptance remain outside this release.

## 0.3.0-alpha.5 — 2026-11-19

### Added

- Resumable, source-authorized model rebuild jobs with bounded backfill, dirty-ID
  reconciliation, explicit atomic activation, retirement cleanup and cancellation.
- Operator `models rebuild start|step|status|activate|cancel` commands; staged rows
  remain private while the existing revision serves.
- Owned-PostgreSQL recovery/privacy controls and CLI process-restart, source-outage,
  revocation and rollback tests. Large staged LocalNet acceptance remains pending.

### Security

- Reject nonowner schema, relation, sequence and column grants in the private
  destination; verify projection column ACLs as well as table grants.
- Keep source/caller guards through canonical REST encoding; share the final
  before/after encoding guard across private transports.
- Build canonical response JSON only after authorized pagination, rather than
  spooling full payloads for visible-total counts.
- Admit HTTP requests before body extraction, enforce a 30-second deadline,
  retain database slots across cancellation and cap complete JSON responses at 2 MiB.
- Reject PQS lease/plan party mismatches; disable local-helper proxies/redirects
  and atomically replace private generated files without following symlinks.
- Disable persisted checkout credentials and bound the CI verification job.

### Documentation

- Consolidate release history, verification and roadmap pages; remove obsolete drafts.
- Document PoC lifecycle/visibility, exact calculations, helper side effects and Rust contracts.

### Compatibility

- Explicit `storage init` applies migration 006; migrations 001–005 are unchanged.
  Older binaries reject the new migration set. No automatic downgrade or new
  external runtime dependency.
- Atomic `models bind` retains its 128-row cap. Staged steps use 1–64 rows
  (default 32) and an 8 MiB evaluation budget. Activation invalidates old model cursors.

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

## 0.3.0-alpha.3 — core crate publication

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

### Limits at release

- Generated typed APIs, revision-bound cursors and LocalNet model acceptance were deferred to later alphas.
- Installation does not ingest contracts, activate live processing or authenticate API callers.

### Fixed

- Consolidate developer/reference docs, remove obsolete planning and comparison pages, and correct stale status claims.
- Reject PostgreSQL-incompatible NUL text in model fields, optional values, literals and filters.
- Apply the projection-size limit to directly supplied typed rows before SQL encoding.

### Compatibility

- Run explicit `storage init` to apply owned migrations through 005 before opening
  the destination. Migrations 001–004 are unchanged. Older binaries reject unknown
  migration sets; downgrade requires a separate reviewed plan.

## 0.2.0-alpha.1 — 2026-11-17

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

## 0.1.0-alpha.1 — 2026-11-17

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
