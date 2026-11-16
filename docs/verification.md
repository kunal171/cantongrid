# Verification plan and evidence

## Documentation phase evidence

Phase 0 contains documentation and repository housekeeping only. Validation must
check local Markdown links, whitespace, secret/build ignore rules, consistency of
scope and planned-status labels. Record actual results in the phase handoff.
No Rust build, database, source adapter or API test can be claimed for this phase.

## Planned runtime acceptance matrix

These checks are requirements for later phases, not tests already implemented.

| Area | Required checks |
| --- | --- |
| Configuration | Missing credentials, invalid selections, unknown types/operators and incompatible metadata fail clearly without secret leakage. |
| Canonical ingestion | Create/archive and supported other events, duplicates, stable identity and explicit source capability limits. |
| Snapshot/update handoff | Updates during snapshot, pagination, reconnect and interruption cause no missed lifecycle transitions. |
| Atomic progress | Crash before/after commit never separates state, projections, checkpoint and outbox; replay has no duplicate state effect. |
| Data fidelity | Exact numeric/Int64 round trips and aggregates; optional/variant/time mappings; unsupported mappings fail explicitly. |
| Authorization | A/B/AB scope matrix through REST, GraphQL and streams; inaccessible filters/counts/relationships/errors reveal no private data. |
| Derived outputs | Join dependencies, aggregate scope, archival retraction, deterministic replay and versioned rebuild equivalence. |
| Cursor isolation | Tampering, another principal, changed grants/schema/source, expired retention and revoked access. |
| Operational failure | Source failure, expired authorization lease, partial history, PostgreSQL outage and worker concurrency. |
| Schema evolution | Compatible additions; breaking rebuild, controlled promotion and rollback; no upstream DDL or silent data deletion. |
| Durable delivery | Outbox survives process failure; at-least-once IDs, reconnect, lag and explicit resync after retention gap. |
| Genericity | Outcomia and an unrelated application work through public config with no core/resolver/migration edits per model. |
| CLI | Init placeholders, validation, restart, once-boundary semantics, orderly shutdown and separate index/serve operation. |

## Test levels and release claims

Fixtures verify engine semantics without proving upstream behavior. PostgreSQL
integration tests verify persistence and transaction boundaries. LocalNet checks
verify the selected source and real Daml data flow. Devnet checks apply only when
authorized infrastructure is available. Report each level separately.

An end-to-end acceptance run must exercise authorized source data through
persistence, processing, authenticated query and durable delivery. Capture
nonsecret reproduction steps, versions, coverage, expected results and observed
limitations. Do not commit private payloads or credentials as evidence.

Capacity tests must measure representative model sizes, history, API queries and
stream consumers. Publish the environment and practical limits with results;
do not turn an unmeasured design into a scalability claim.

## Phase 0 handoff — 2026-11-16

Completed documentation checks:

- All 17 relative links in 12 Markdown documents resolve; code fences balance.
- Git whitespace checks pass across the complete documentation change.
- Secret/build paths are ignored; `.env.example` and documentation remain eligible
  for tracking. No secret values were used for this check.
- Tracked files contain only Markdown and `.gitignore`; no remote is configured.
- Both existing Outcomia repositories retain their prior state, including the
  preexisting untracked `.vscode/` directory in outcomia-canton.

This is documentation evidence only. No executable, LocalNet, database or API
verification has been performed for CantonGrid.


## Local infrastructure handoff — 2026-11-16

LocalNet startup, read-only authenticated API checks, stop/start persistence and
HTTP UI availability are recorded in [LocalNet evidence](localnet-evidence.md).
Python parsing, local Markdown links, generated configuration isolation and Git
whitespace checks pass. These development helpers do not implement the planned
CantonGrid CLI, connector or API server. Runtime acceptance matrix items above
remain future work unless explicitly covered by that evidence.
