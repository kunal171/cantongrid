# Changelog

## 0.1.0-alpha.1 — unreleased

### Added

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
