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


## Compatibility handoff — 2026-11-17

[Outcomia DAR validation](outcomia-compatibility.md) and
[PQS SQL/source/recovery checks](pqs-compatibility.md) passed on the recorded
LocalNet profile. An invalid DAR was rejected. The PQS probe rejected a stopped
source despite retained database rows. Recreating it preserved an explicitly
pinned historical snapshot, caught up to newer source updates and did not
produce duplicate create identities. These are upstream fixture checks, not
CantonGrid runtime, authorization-isolation or outbox acceptance.


## SQL reader follow-up — 2026-11-17

The fixture now has a separately authenticated database reader. Active and
historical PQS queries pass under it. Table writes, public schema writes,
temporary tables and assuming the owner role fail with insufficient-privilege
SQLSTATE 42501 even after disabling the default read-only transaction setting.
The reader setup is operator-side configuration of the owned development
fixture; the future Rust source adapter remains read-only.


## Visibility and upstream triage — 2026-11-17

The owned Ledger API fixture passed A/B/AB visibility, forbidden cross-scope
filters, preissued/fresh-token revocation, exact scalar and grant-restoration
checks. The dedicated AB PQS source preserved exact payloads/witnesses and denied
its database to the original reader. Stream readiness loss and retained B-only
cache rows were measured, not interpreted as a complete authorization mechanism.

The schema-collision reproduction ran without Canton/application code against
both the packaged and current-upstream V001 SQL: clean controls passed, unrelated
type collision failed as expected, and a namespace-scoped in-memory correction
passed both controls. No full current executable/Scala suite was claimed.


## Rust core foundation — 2026-11-17

Twelve unit tests passed for source-neutral visibility, derived-input dependency
retention, empty-evidence denial, source/generation mismatch, identity redaction,
source/caller grant intersection, bounded lease expiry, earlier credential expiry
and explicit revocation. All run without source credentials or network access.
`cargo test --workspace --offline`, Clippy with warnings denied, and formatting
checks passed using Rust/Cargo 1.100. No external crate dependencies were added.
These are pure decision-model tests; live connector/serving integration remains open.

## Phase 2 handoff — 2026-11-17

Completed the Rust configuration and fixture pipeline milestone on
`feat/core-config-fixtures`, in focused configuration, values, lifecycle, examples,
fixture decoding, CLI, helper consolidation and CI commits.

Implemented and checked:

- Strict project/fixture/model/payload versions, explicit source/network/scope
  generation, exact template selections and declared create/archive/history
  capabilities. Unknown fields, adapters, event kinds, unsupported processing,
  duplicate keys and numeric float tokens are rejected without raw diagnostics.
- Tagged exact values including Numeric 10 and Int64 boundaries, optional/enum
  values and nested records/lists. Validation rejects range/scale/precision and
  unsupported-type errors. Full metadata/type checking and arithmetic are deferred.
- Atomic in-memory batch publication, deterministic replay, conflict detection,
  stable source-local identities and opaque positions. Invalid batches leave
  canonical state/history and checkpoint unchanged. Archive-without-create,
  repeated archive and contract reuse fail; missing rows never imply archival.
- Active/history reads enforce caller/source grant intersection, visibility
  dependencies and lease validity. Tests cover A/B/AB, no grants, revocation,
  expiry, source/network/generation mismatch and archive dependency retention.
- Executable `init`, `validate`, and `fixture run` workflows pass from a separate
  working directory, including refusing scaffold overwrites, relative fixture
  resolution, redacted failures, bad inputs, regular-file and size checks.
- Both unrelated Visibility and Outcomia Protocol fixtures use one generic engine.
  Each replay applies two batches initially and zero on repeated delivery.
  Expected scoped counts are documented with the examples.

Validation: **29 Rust tests** (26 core + 3 executable workflow tests), **6 isolated
Python helper regressions**, formatting, Clippy with warnings denied, Python syntax,
Markdown links and Git whitespace checks passed. Rust checks use the committed
lockfile and run offline on Rust/Cargo 1.100. A GitHub Actions workflow was added
for these credential-free checks; it has not been run remotely in this handoff.

Development helper review replaced ten standalone executable scripts with one
`dev.py` entry point and grouped modules, merged the PQS lifecycle implementations
and shared Compose/SQL/health/private-file helpers. Purposeful independent checks
and the standalone upstream reproduction were retained. No new upstream defect
was found in this work. The test harness signature error encountered during
refactoring was ours and corrected before this handoff.

The refactored helpers were also exercised against the existing LocalNet:

- All three validators/authenticated Ledger APIs passed; unauthenticated access
  was rejected. The SQL reader still queries while DML, DDL, temporary-table and
  owner-role probes fail with SQLSTATE 42501.
- Both sources initially failed readiness. Recreating them with renewed one-hour
  credentials for the existing scoped users restored readiness, consistent with
  credential expiry. Source startup now waits for readiness, avoiding the observed
  connection reset while the recreated process starts. No source processing
  defect was demonstrated.
- Historical offset 80 remains active 11/create 17/archive 6, with no duplicate
  creates and all create witnesses in scope. Exercise coverage remains zero.
- A/B/AB Ledger checks passed with exact values and restored grants. AB PQS cache
  checked exact witnesses/payloads and denied the unrelated SQL reader. Source
  readiness loss was observed after 2.02 seconds during this revocation run;
  three cached records, including B-only, remained. Grants were restored and the
  AB source recreated/ready. This measurement is not a revocation SLA.
- The previously recorded Outcomia DAR digest was validated again without upload.
  No Outcomia ledger transactions or wallet changes were performed.

Limits: the Rust workflow is fixture-only, with synthetic input and simulated
caller grants. State/checkpoints are process memory; it does not provide durable
restart, PostgreSQL, a live connector, authenticated APIs, generated processing
models or streams. Phase 3 must resolve source handoff, production permissions,
current grant retrieval and live lifecycle/identity/coverage gates before copying
private data. Do not infer those guarantees from the fixture or helper checks.

## Owned PostgreSQL feature handoff — 2026-11-17

Implemented the first Phase 3 storage slice on `feat/pqs-ingestion-storage`.
This is a bounded local reference feature using synthetic fixtures, not completed
live ingestion or authenticated serving.

Delivered in focused commits:

- Versioned persistence envelopes preserve original source/event/witness evidence;
  current grant decisions are never deserialized. Canonical plan definitions bind
  each source/network/generation to its exact selection/schema context.
- A separate PostgreSQL 14.25 destination on loopback 5442, its own volume/network,
  restricted owner role and checksum-guarded explicit schema initialization.
  Source connection, privileges, database and migrations remain separate.
- Atomic journal, canonical contracts/history, checkpoint/revision and outbox
  publication, source-context writer exclusion, deterministic recovery and replay.
- Durable `fixture index/status/query` and explicit `storage init` commands,
  grouped separately from read-only `source inspect-pqs`. Both fixture examples
  use the same implementation; caller grants remain explicitly simulated.
- CI now starts the owned PostgreSQL fixture and runs transaction/recovery and
  executable tests. The actual LocalNet/PQS profile check remains separate.

Validation passed locally on the committed dependency lockfile:

- **42 Rust tests**, including all PostgreSQL-dependent tests and the real source
  inspection test (`cargo test --workspace --locked --offline -- --include-ignored`
  with the dedicated local fixture environment variables).
- **6 Python helper tests**, Python syntax, Rust formatting and Clippy with warnings
  denied; workflow/Compose YAML parsing; 27 Markdown documents/71 relative links
  at this handoff; Git whitespace checks.
- Reopening destination connections preserves exact Numeric 10/Int64 values,
  A/B/AB visibility, archive dependencies, lease invalidation and checkpoint order.
  Replaying committed input produces no extra state/history/outbox effects.
- A real SQL error injected after staged writes rolls back canonical state,
  lifecycle events, outbox and progress together; reconnect/retry succeeds once.
  A second worker is denied while the first transaction holds the source lock.
- Selection changes within a generation, conflicting/invalid deliveries, oversized
  input, migration-checksum drift and inconsistent checkpoints fail closed.
- CLI queries work after removing the original fixture file. Outcomia Protocol
  input persisted two batches/two contracts (one active), three events and three
  outbox entries; replay applied zero batches. After stopping/recreating the
  destination container with its volume retained, the same counts remained.
- Manual role-mix checks rejected destination initialization against the existing
  PQS reader connection and rejected PQS inspection using the destination owner.
  Failures printed no connection settings, credentials or private payloads.

The Rust source inspector read the existing schema-041 source in a read-only,
repeatable-read transaction: oldest 0/latest 943, no pruning marker, 10 active
rows/132 create rows/122 archive rows at that observation. All 122 archive rows
had empty witnesses, consistent with the installed/upstream function definition.
These are row counts (potentially including interface projections), not a claim
about global contract counts, current stream health or complete authorized history.
No source payloads were copied into the destination; no ledger transactions,
party/DAR provisioning or Outcomia wallet changes were performed.

Failure classification: the archive witness omission is documented source behavior,
not a newly demonstrated upstream defect. The compile-time digest formatting and
test cleanup lint failures encountered during development were local implementation
issues and were corrected before validation. No new public issue/PR was opened.

Limits remain explicit in [storage](storage.md): local-only transport, bounded
journal replay, fixture-only caller simulation, no production retention policy,
no live source-grant verifier/handoff/type normalization and no application APIs
or stream delivery. Private live ingestion remains disabled pending those gates.
The new GitHub workflow has not been run remotely for this branch yet.

## Source contract, security and alpha candidate — 2026-11-17

[PR #3](https://github.com/kunal171/cantongrid/pull/3) merged at `1007134b5001`
after both fixture/PostgreSQL CI runs passed. Newer ecosystem documentation on
main was preserved. Release-oriented work continues on `release/0.1.0-alpha.1`;
this reference alpha is distinct from full Phase 3 acceptance.

Delivered and locally checked:

- Reviewed the supplied v0.3 handoff and preserved it with a historical-baseline
  note. [Source contract](phase3-source-contract.md) records installed SQL,
  visibility/retention restrictions and the SDK adoption decision before live work.
- SDK v0.3.2, commit `5caaebc3eab380de827196663d4bb18b90ffd6f4`, decoded 3 creates,
  3 archives and all 11 historical offset-80 active rows using a restricted read-only
  connection. Eight historical rows had a later archive; the SDK retained them
  correctly. No SDK writer/admin suite ran and no runtime dependency was adopted.
- Retained interval 80→943 reconciled with baseline 11/creates 115/archives 116/
  active 10 and no identity differences. Empty windows, invalid/regressed/future
  bounds and pruning gaps have tests. Concurrent publication/pagination/reconnect
  remains unverified; the probe explicitly reports that limit.
- Machine-readable source coverage remains unknown/partial with reason codes;
  snapshot availability does not imply current source permission or freshness.
- Shared SQL active/history pages filter before totals and paging, preserve every
  visibility dependency, reject empty evidence and recheck source leases before
  returning. A/B/AB/no-party and one-row pagination tests passed on owned PostgreSQL.
  This is a backend foundation using fixture grants, not real API authentication.
- Contributor/security guides, release milestones, changelog, PR template and a
  version/license/notice guard are added. The agreed Apache-2.0 target now has an
  actual LICENSE and NOTICE; all workspace packages inherit it and the alpha version.
- Pinned security tools verify both archive and executable hashes. The checker
  rejects tampered binaries, ambiguous/linked archive members and tracked symlinks.
  Explicit scanner configuration prevents ambient config/ignore/comment bypasses.
  Full Git history and Git-eligible working files pass redacted secret scans.
- RustSec audit passed with no advisories; dependency licenses (including workspace
  and development packages), registry sources and version constraints passed. Two
  transitive WASI versions produce an informational duplicate warning, not a denied
  advisory. There are no blanket scanner/advisory or private-package license ignores.
- New security CI runs on push/PR, manual dispatch and weekly with read-only
  permissions. actionlint passed locally; GitHub execution is recorded on the PR.

Local final baseline: **49 Rust tests** including all environment-dependent cases,
**11 Python tests**, formatting, Clippy with warnings denied, YAML parsing, links,
release metadata and security checks passed. SDK evaluation is additional isolated
compatibility evidence, not counted as a workspace unit-test suite.

The SDK build exhausted root-disk space and interrupted two documentation writes.
Only this task's generated Cargo targets were cleaned, interrupted documents were
restored, and `git fsck` plus link/nonempty-file checks passed. Subsequent builds
use a private temporary target on tmpfs with debug symbols/incremental output
reduced; unrelated project files and caches were not deleted.

Remaining live gates and runtime dependency proposals are explicit in the source
contract. No Outcomia deployment, new parties, source database migration or ledger
transaction was performed. Phase 3 is not marked complete by this alpha candidate.
