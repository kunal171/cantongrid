# Verification

These are recorded results, not tests rerun whenever this page changes.
The supported environment is LocalNet; devnet and production are unverified.

## Environment

| Component | Tested version / boundary |
| --- | --- |
| Rust/Cargo | 1.100.0 |
| Python helpers | 3.11+ |
| CN Quickstart | `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9`, Splice 0.6.11 |
| Participant / PQS | Canton 3.5.7, Scribe 3.5.4, SQL schema041 |
| Database | PostgreSQL 14.25; separate source and owned destination |
| Daml example | SDK 3.7.1, LF target 2.1; not a general SDK/runtime compatibility claim |

## LocalNet contracts — 0.2.0-alpha.1

Verified 2026-11-17; [PR #5](https://github.com/kunal171/cantongrid/pull/5)
and its main-branch CI passed. Source release:
[v0.2.0-alpha.1](https://github.com/kunal171/cantongrid/releases/tag/v0.2.0-alpha.1).

| Check | Result and limit |
| --- | --- |
| Source permissions | Exact CanReadAs scope accepted; forged source-token signature rejected by Ledger. SQL readers denied writes, DDL, temporary objects and owner escalation. |
| Snapshot/update paging | Controlled Outcomia publication produced all eight expected create/archive identities with one-event pages and 17 worker invocations. Snapshot-forward history remains partial. |
| Atomicity/recovery | SQL failure rolled back state/history/checkpoint/outbox; duplicate delivery had no new effects; stale writers could not rewind progress; live path passed beyond the fixture's 256-batch cap. |
| API visibility | Actual Visibility active counts A=2, B=2, AB=3 through REST/GraphQL. Tested identity isolation, paging, cursor tampering/principal/kind/epoch reuse, token signature/issuer/audience/expiry and local caller revocation. |
| Source outage | Stopped exporter with rows still in SQL: no worker advancement, APIs returned 503; restart restored access. Source leases are bounded to 15 seconds. |
| Exact values | Visibility Numeric `12345678901234567890.1234567891` and Int64 `9223372036854775807`; Outcomia tick `0.0000000001` and maximum lot retained as strings through PQS/PostgreSQL/APIs. |
| Provenance | Public history retains original offsets/times, snapshot status and prior-create policy; omits operator witness sets and source PKs. Pruned/regressed cursors fail. |
| Genericity | Actual Outcomia Protocol/BinaryMarket and unrelated Visibility use the same reader/store/APIs, with configuration changes only. |

At that milestone, 47 credential-free and all 14 environment-dependent Rust tests
passed (61 total), plus 13 Python tests, formatting, Clippy, release/security and
container-retention checks. LocalNet reports stay ignored under `.local/`.

### Source observations

- Retained interval 80→943 reconciled 11 initial + 115 creates − 116 archives =
  10 active rows, with no missing/unexpected identities. This historical control
  is separate from the concurrent publication test above; offsets are not constants.
- `set_latest(NULL)` / `set_oldest(NULL)` returned NULL in schema041. The adapter
  resolves explicit bounds inside its transaction. Archives have empty witnesses.
- Nodejumper SDK v0.3.2, commit `5caaebc3eab380de827196663d4bb18b90ffd6f4`, decoded
  three create, three archive and eleven offset-80 active rows. Eight had later
  archives and correctly remained active at 80. No SDK runtime dependency was added.
- Fresh Ledger requests respected revoked B rights with a preissued token. PQS
  cached B-only rows remained after revocation; readiness loss took about 1–5 seconds
  in two observations. Those timings are not a revocation guarantee.

### Outcomia artifact

The explicitly approved local test used the existing DAR with SHA-256
`eef7340eba83f4f2af4865258c66c6461876bacb05b8e4dbc6c7ca74459f6f0e`,
main package `73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a`.
It uploaded the DAR and created an isolated Protocol/Draft/Open/Closed lifecycle.
No funded collateral, orders, trades or resolutions were created. The sibling
source repository was not modified or asserted to have been rebuilt.

## Model tools — 0.3.0-alpha.1 in progress

[PR #6](https://github.com/kunal171/cantongrid/pull/6) and post-merge Rust/PostgreSQL
and security CI passed. Both model examples share validation/evaluation/SQL planning.
Controls cover exact arithmetic, overflow, filters, missing/optional values,
invalid names/paths/definitions and deterministic fingerprints. Owned-Postgres
DDL/row round trips run in a rollback-only transaction after checking fixture identity.

The initial compiler pass ran 54 credential-free and all 15 environment-dependent
Rust tests (69 total), plus 13 Python tests. Review then added a regression for
PostgreSQL-incompatible NUL text and oversized direct rows: all 55 credential-free
tests and the affected model PostgreSQL test passed. The other 14 integration tests
were not rerun for that model-only fix. Formatting and Clippy passed.

Offline guide checks ran scaffold/fixtures, both model previews/plans and a custom
`1.20 × 2 = 2.40` example. JSON, shell syntax and links were checked. Live guide
commands were checked against code and earlier evidence, not rerun for prose edits.
At that milestone, model storage, lifecycle integration and generated APIs were
not implemented; the projection checks below cover the later storage work.

## Model registry

The registry milestone passed 56 credential-free Rust tests and all 18 owned-
PostgreSQL tests, including CLI redaction and transport privacy controls.
Registration tests cover both examples, immutable/idempotent revisions, namespace
isolation/locking, whole-catalog rollback and stored-content drift. Migration
controls verify explicit 002→003 upgrade, checksum/unknown-version rejection and
rollback. Formatting, Clippy, security, release and documentation checks passed.
The three source-dependent tests were not run for this change. No Ledger writes,
projection installation or live model activation was performed.

## Projection-table installation

The installer passed 56 credential-free Rust tests and all 30 owned-PostgreSQL
tests. Both examples round-trip exact types, including Numeric scales 0/10/37.
Controls cover retries/sharing, private grants, schema/key/index/default drift,
rules/triggers/inheritance, missing/unmanaged tables, metadata collisions, DDL
locking and real SQL rollback of table/metadata/link writes. CLI errors omit literals.
Formatting, Clippy, security, release and documentation checks passed; the registry
yanked-package lookup succeeded on retry. Three source-dependent tests were not run.
Installation does not activate models, copy contracts or complete Phase 4.

## Source-bound live projections

This slice passed 56 credential-free Rust tests, 44 owned-PostgreSQL checks and
13 Python helper tests. The source-bound projection tests use synthetic authority,
not a live Canton source. Both model examples retain exact values through bounded
backfill, forward creation and archive. Controls cover filtered/nullable rows,
whole-page rollback, schema drift, compare-and-replace promotion/rollback,
idempotent retries, lease expiry after writes, source/selection mismatches,
writer exclusion, aliases and cross-stream isolation. Limits cover 129-row
rejection, 16 bindings and a shared 8 MiB work budget. The CLI denies activation
when source rights/readiness fail. Migration 005 upgrade/drift checks preserve
existing installed tables and all earlier migration SQL.

Formatting, Clippy, security, release and documentation checks passed. The three
source-dependent tests were not run; actual LocalNet model activation/lifecycle
acceptance and generated model queries remain pending. No Ledger writes or
publication were performed for this slice.

## Standalone LocalNet PoC (0.3.0-alpha.2)

On 2026-11-18, two complete runs deployed `Market:PredictionMarket`, indexed its
Draft/Open/Resolved lifecycle and verified generated model GraphQL. The first
run matched seven real event identities; the repeat matched thirteen accumulated
identities after archiving the previous resolved market, without resetting volumes.
Both checked exact numeric/Int64 values, A-only/B-only/shared visibility, cursor
isolation, caller revocation, a choice submitted while the worker was stopped,
restart catch-up and no duplicate history on repeated observation. Generated
TypeScript compiled; Chromium rendered the local viewer. API proxy checks returned
2/2/3 rows for A/B/both. See the [runner and sanitized evidence](guide/poc.md).

Credential-free checks also cover generated nullability/revision identity and safe
PoC cycle replacement. Owned-PostgreSQL controls exercise typed API authorization,
null cursors, namespace/operation limits and source health/permission failures.
These runs were recorded before release and are included as evidence for
0.3.0-alpha.2. The original report is retained in that release tag; the current
artifact adds the alpha.4 query evidence and separates the earlier viewer check. No grant submission is included.

## Source defect

An unrelated schema's `__event_type` prevented PQS store initialization. The
[standalone reproduction](https://github.com/kunal171/cantongrid/blob/main/repros/pqs-schema-type-collision/README.md) isolates
upstream SQL from CantonGrid processing. Tracking:
[upstream issue #240](https://github.com/digital-asset/participant-query-store/issues/240),
[PR #241](https://github.com/digital-asset/participant-query-store/pull/241),
[CantonGrid issue #1](https://github.com/kunal171/cantongrid/issues/1).
The proposed callback fix leaves versioned migrations unchanged. PostgreSQL 14.25
controls passed; the full Mill suite was not run locally. These links record the
contribution, not a claim about its current upstream merge status.

## Limits

No remote/TLS, alternate identity provider, interface/reassignment lifecycle,
complete pre-snapshot history, automatic erasure, model joins/aggregates, durable
streams, devnet or production acceptance is claimed. Fixture runs are synthetic.
For new failures, separate our configuration/processing from documented source
behavior, then reproduce any suspected upstream defect with nonsecret controls.


## Model queries — alpha.4 development

Verified 2026-11-19 on the same pinned LocalNet profile. Two complete query runs
matched 23 and 29 cumulative controlled event identities; each checked 54 page
sequences across A/B/AB and REST/GraphQL. Four reusable query contracts provide
prices 1/2/10, equal-value ties, Int64 quantities above 2^53 and null/false/true
outcomes. Every page checks visible count, contract identity, ordering and exact values.

Discovery and equivalent cross-transport cursors matched. Different callers,
filters, sorting, grant epochs and active revisions rejected old cursors. Revision
2 promotion and rollback preserved projections; failure-path unit controls check
restoration after assertion and ambiguous promotion failures. The repeat run also
stopped the exporter: retained rows returned 503, then recovered without checkpoint
movement or duplicate effects. A separate live check verified monotonic grant
restoration and rejection of pre-revocation cursors. Generated TypeScript compiled.

Evidence: [sanitized report](../examples/poc/evidence.json), with the tested code
commit and environment. Chromium viewer evidence remains from alpha.2; it was
not repeated for alpha.4. This does not complete Phase 4: the application check below extends lifecycle
acceptance, while large staged rebuilds and funded trading projections remain open.


## Outcomia model queries — alpha.4 development

Verified 2026-11-19 using the approved Outcomia DAR identified above, with no
sibling source changes or Rust engine special cases. The [sanitized application
report](https://github.com/kunal171/cantongrid/blob/main/examples/fixtures/outcomia/model-query-evidence.json)
records the tested commit. The complete run passed 60 one-row page sequences,
25 exact submitted create/archive identities and seven current contracts.

A-only, B-only and shared BinaryMarket contracts covered Draft/Open/Closed,
lot sizes 2/10/9007199254740993, exact ticks and derived basis-point filters.
REST and generated GraphQL matched discovery, counts, values and cursor behavior.
Caller/filter/sort/revision changes rejected cursors; disabling B denied its
bearer while A retained its permitted view. Grant restoration advanced the epoch.
Revision 2 added the real optional closure reason, checked null/non-null filters
and nulls-last sorting, then restored revision 1. Generated TypeScript compiled.

The earlier run passed 42 narrower page sequences. An intervening repeat stopped
because the exporter's one-hour local token expired (`stream_up=false`); renewal
restored export. This was fixture credential expiry, not an upstream defect. The
helper now preflights readiness before uploading/submitting and follows all API
pages; the complete run needed 26 worker invocations rather than the old fixed 17.
Nineteen development-helper tests and four PoC helper tests pass. Staged large
backfills and funded trading projections remain outside this acceptance result.


## Staged model rebuilds — alpha.5 development

Verified 2026-11-19 with owned PostgreSQL 14.25, synthetic source leases and a mock
HTTP authorization/readiness server. This is not a large staged LocalNet run.

Six storage regressions cover a 261-row model beyond the atomic limit, resumed
pages, creates behind the scan cursor, archives before/after scanning, changes
after readiness, private staging, cancellation, alias/stream isolation and writer
lock contention. Real SQL failures during backfill and activation, plus lease
expiry after writes, preserve both durable progress and the prior binding.
The active model keeps ingesting when a staged expression fails.

The CLI integration test uses separate processes to resume, reject source outages
and revocation, promote, invalidate old cursors, retain caller privacy, clean up
and rebuild back to the old revision. Migration controls verify explicit 005→006
upgrade and reject checksum drift and public job-table grants; 001–005 stay intact.

All 59 credential-free Rust tests and 53 owned-PostgreSQL/HTTP tests passed, plus
19 development-helper and four PoC helper tests. Formatting, Clippy, release
metadata and container log-retention checks passed. Source-dependent PQS tests
and the existing LocalNet application scenarios were not rerun for this change.


## Security regression pass — alpha.5 development

Verified 2026-11-19: 63 credential-free Rust tests, 55 owned-PostgreSQL/HTTP tests,
23 development-helper tests and four PoC helper tests passed. Two ACL controls
first failed against the previous implementation and passed after the fix.
Transport controls use a loopback test server; no Ledger writes occurred.

RustSec, dependency license/bans/source policy, full-history and eligible-tree
secret scans, actionlint, formatting, Clippy, release metadata and container
retention checks passed. See [findings and remaining boundaries](security.md#repository-security-review--2026-11-19).
Existing source-dependent PQS and LocalNet application scenarios were not rerun.


### Follow-up review

The second pass fixed canonical REST authorization after encoding and prevented
canonical active/history counts from building all visible payloads before LIMIT.
A PostgreSQL evaluation counter failed on the old query and passed with both
rewritten queries. A serializer that revokes access during encoding returns only
the denial body. Failed-ingestion controls preserve a ready rebuild's dirty queue,
canonical/model rows and checkpoint through later activation.

All 64 credential-free Rust tests, 57 owned-PostgreSQL/HTTP checks and 27 Python
tests passed. Formatting, Clippy and security scans passed. The PostgreSQL 14
support-status follow-up is recorded in [security](security.md); source-dependent
PQS and large staged LocalNet acceptance were not rerun.
