# Verification

Recorded results below are dated evidence, not tests rerun when this page changes.
The current implementation and remaining work are listed in the [overview](README.md).

## Environment

| Component | Tested version |
| --- | --- |
| Rust/Cargo / Python helpers | 1.100.0 / 3.11+ |
| CN Quickstart / Splice | `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9` / 0.6.11 |
| Participant / PQS | Canton 3.5.7 / Scribe 3.5.4, SQL schema041 |
| Database | PostgreSQL 17.12, separate source and owned destination; historical alpha runs used 14.25 |
| Daml example | SDK 3.7.1, LF target 2.1 |

This is the tested LocalNet profile, not general SDK/runtime compatibility.
PostgreSQL 14's support status and the upgrade requirement are in [operations](guide/operations.md#postgresql-support).

## Offline reconciliation policies — 2026-11-23

Unreleased `0.5.0-alpha.1` on `feat/reconciliation-policies`. This implements
versioned policy compilation and explicit fixture evaluation only; every report
remains unverified. See the [guide](guide/reconciliation.md) and
[live-track gate](design/reconciliation.md#live-track-gate).

| Check | Result |
| --- | --- |
| Rust | 139 default workspace tests pass; formatting and strict all-target Clippy pass |
| Policy controls | 11 tests cover canonical identity, strict parsing, work bounds, exact comparisons, negative zero, scale mismatch, missing inputs, arithmetic overflow and redacted failures |
| CLI | Offline integration passes outside the checkout with unusable live credentials, unchanged inputs and rejected extra arguments |
| Existing behavior | 82 storage, one model/PostgreSQL and 15 HTTP integration tests pass against the owned destination; existing model validation and projection output match the stable 0.4.0 CLI |
| Helpers/security | 38 development-helper and seven PoC-helper tests pass; RustSec, dependency policy, actionlint and history/tree secret scans pass |
| Packaging | All ten workspace package dry runs and archive checks pass, including README, legal files and self-contained fixture assets; nothing uploaded |

No new dependency, migration or live-source verification is included. SQL tests
use the separate owned destination; no LocalNet transactions or source-grant
changes are performed. Fixture comparisons do not establish complete source
coverage, authorized live balances or economic correctness of a supplied formula.

## Offline model-policy preflight — 2026-11-23

Same unreleased branch/version as the policy slice. Preflight validates one exact
projected model schema and produces a domain-separated definition identity; it
checks no active binding, source authorization or indexed contract data.

- All 145 default workspace tests, formatting and strict all-target Clippy pass.
  The policy suite now has 16 tests, including five preflight controls; two CLI
  integration tests pass outside the checkout with unusable live credentials.
- Controls cover exact types/scales, nested-vs-projected paths, missing/optional
  inputs, constant-only checks, revision/selection/namespace changes, catalog
  ordering, explicit model selection, redaction and strict/bounded file inputs.
- All ten workspace package dry runs and archive checks pass at `9c8cd13`,
  including the packaged model example and test assets. Nothing is uploaded.
- Existing SQL/runtime code, migrations and dependency versions are unchanged.
  The previous owned-PostgreSQL evidence above is not a new live-source run.

At that check, indexed-contract evaluation was still unimplemented. Its separately
authorized plan is in the [design](design/reconciliation.md#indexed-contract-slice-implemented-owned-postgresql-evidence).

## Indexed per-contract reconciliation — 2026-11-23

Unreleased `0.5.0-alpha.1` on `feat/reconciliation-policies`. Storage-only
`Store::read_reconciliation` checks one exact active model over a complete
retained indexed prefix. These results use owned PostgreSQL 17.12 and synthetic
source/caller authority, not LocalNet or remote Canton evidence.

| Check | Result |
| --- | --- |
| Default Rust | All 145 workspace tests, formatting and strict all-target Clippy pass |
| SQL/runtime regression | All 92 storage tests, one model/PostgreSQL test and 15 HTTP integration tests pass |
| Packaging | All ten workspace dry runs and self-contained archive checks pass at `9fddaee`; no upload |
| New reconciliation controls | Ten owned-database tests cover exact lifecycle/filter/replay, redaction, scope-before-membership denial, wrong source context, raw-path rejection, projection drift/missing/extra rows, complete/stale/pruned/unknown coverage, selection floors, empty sets, 128/129 input limits and byte budgets |
| Concurrent reads | A separate connection archives/creates and promotes a model while an in-flight read retains its original canonical inputs and model revision; a later read rejects the old binding |
| Source expiry | A barrier proves the read snapshot was acquired before the lease expires; the completed result is discarded |
| Exact arithmetic | Values above 2^53 and last-digit differences remain exact; valid native Numeric columns that overflow policy arithmetic produce an unavailable overall comparison |
| Security | RustSec, dependency license/bans/source checks, actionlint and history/tree secret scans pass; no new dependency or migration |

Reports contain configured identities and string counts, with no payload values,
contract IDs, witnesses or global source offsets. Coverage is independent from
arithmetic outcome; complete inputs can still yield unavailable calculations.
No public route, source-grant change, DAR deployment, ledger transaction, persisted
result registry, cross-contract total or financial certification is included.
See the [guide](guide/reconciliation.md#indexed-per-contract-checks-rust-storage-api).

## Authenticated reconciliation API — 2026-11-23

Unreleased `0.5.0-alpha.2`, branch `feat/reconciliation-api`. The operator manifest
and endpoints are implemented; all evidence below uses owned PostgreSQL 17.12
and mock read-only source authority, with no Canton transaction or grant change.

- All 147 default workspace tests and strict all-target Clippy pass. Two new
  manifest tests cover bounds, strict documents, relative files, immutable pins,
  alias reuse and sanitized errors without a source/database connection.
- All 16 HTTP integration tests pass. The reconciliation control checks typed
  REST/GraphQL equality and discovery, the packaged query/variables, no-store,
  redaction, scope denial before existing/unknown lookup, strict `{}` bodies,
  policy pins, active promotion, stale/pruned evidence and arithmetic unavailable.
- A counted mock-source barrier changes policy files, caller grants, source
  permission and model revision during the final refresh, after evaluation.
  Every case discards the old result; GraphQL resolver/complexity errors return
  no sibling data. The barrier firing is asserted independently of the status.
- RustSec, dependency license/bans/source checks, actionlint and history/tree
  secret scans pass. All ten package dry runs and self-contained archive checks
  pass at `d99e96e`, including the query asset and package READMEs; no upload.
- Storage/evaluator source and migration SQL are unchanged from published alpha.1;
  its lower-level SQL evidence above is not claimed as a new run here.

At that check, real LocalNet/remote reconciliation remained unverified. This branch is separate
from released alpha.1; no alpha.2 release, crate upload or public site update is
performed by these checks. See the [API guide](guide/reconciliation.md#authenticated-api-050-alpha2).

## Read-only LocalNet reconciliation — 2026-11-23

[Sanitized evidence](../examples/reconciliation/localnet-evidence.json) records
clean commit `8476c0b`, the actual CLI binary hash and seven existing PoC contracts.
Ledger and restricted PQS active inventories match at the same completed indexed
boundary, including exact Numeric/Int64 terms. The isolated destination uses the
existing model in a new namespace; original PoC files and bindings stay unchanged.

REST and typed GraphQL agree for `priceTick * 10000 == priceTickBps` and
`quantityLot > 0`. Deliberately changing the policy multiplier to 10001 makes all
seven basis-point checks violated; restoring the policy restores the original
report. Narrow-scope existing/unknown-name denial, isolated application-caller
revocation and API restart pass against real current source authorization.
No DAR upload, party allocation, Ledger command or Ledger-rights change occurs.
These term checks do not test collateral, trading, cross-contract totals or
remote/TestNet sources.

The shared default operations acceptance also passes on seven retained contracts
after the helper refactor (commit `4e7006f`). All 50 offline helper tests pass
(38 development, seven PoC and five operations). CI now runs both example suites.
RustSec, dependency/license policy, actionlint and history/tree secret scans pass.
Private runtime/configuration/identity data remain in `.local`.

## 0.4.0 promotion — 2026-11-23

Regular release of the [scoped node-local profile](releases.md#040-supported-scope-and-acceptance).
Runtime code, tests, packaged fixtures and migration SQL are unchanged from alpha.7;
only coordinated versions and documentation change. Alpha.7's CI and per-feature
evidence below remain the implementation evidence, with CI/package checks rerun
for the stable metadata. Published registry/CLI verification is attached to the
[GitHub release](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0).
The optimized CLI is built from the same clean source; its offline smoke and hash
are recorded separately from registry installation in that report.

The release does not complete the pending M9 remote/production gates. Experimental
temporal limits, unsupported lifecycle types and all evidence distinctions remain
in force. A regular version does not imply full Phase 4 or production acceptance.

## Atomic canonical bootstrap — recorded 2026-11-23

Alpha.7 acceptance on `feat/atomic-snapshot-handoff`. Migration 012 privately
pairs a canonical baseline with its durable feed under the existing writer lock.
Public registration/snapshot responses and signed cursors omit the source boundary
and global queue ordinals. This does not add model CDC or historical model bindings.
See the [guide](guide/bootstrap.md) and [design](design/atomic-snapshot-handoff.md).

| Check | Result |
| --- | --- |
| Rust quality | 127 default workspace tests, formatting and strict all-target Clippy pass |
| Owned PostgreSQL | All 82 storage tests and 15 HTTP/API integration tests pass |
| Bootstrap storage controls | Interleaved archive/create from a separate connection, stable paged baseline, retry identity, ACK/restart, shared writer exclusion/caps, hidden-only pruning, explicit visible loss, expiry, unavailable startup/history, source-scope denial and raised package floors pass |
| HTTP controls | Pinned snapshot across updates/restart, matching post-baseline feed, opaque progress, cross-cursor/owner denial, invalid input, source/caller revocation and explicit retention gaps pass |
| Migration | Intact alpha.6 requires explicit migration; consumer rows persist, new metadata defaults null, and non-owner baseline-column grants are rejected |
| Real retained LocalNet | Seven authorized PoC contracts match the bootstrap baseline across three pages; API restart between pages and paired feed exclusion pass |
| Example | Strict TypeScript compilation and three offline Node tests pass: full staging before publication, no ACK after failed effects, explicit gap handling and rejection of a prematurely terminated baseline |
| Security | RustSec audit, dependency license/bans/source checks, actionlint and history/tree secret scans pass; no new dependencies |

[Sanitized LocalNet evidence](../examples/bootstrap/localnet-evidence.json) identifies
the tested commit. That run submits no ledger transactions and changes no source
grants. Concurrent lifecycle controls use synthetic authority with real owned SQL
and HTTP; real concurrent Daml writes and remote/TestNet acceptance are not claimed.
The complete-boundary and temporal work limits remain in force.

## M7–M9 source checkout — recorded 2026-11-23

Alpha.6 acceptance on `feat/temporal-streams-hardening`. M7 is bounded canonical
as-of state, not historical model bindings. M8 is durable REST lifecycle delivery,
not model CDC or an atomic snapshot/feed handoff. M9 local checks and remote setup
are implemented; actual remote/TestNet and production acceptance remain pending.
The user confirmed no authorized remote source is available.

| Check | Result |
| --- | --- |
| Rust quality | Workspace formatting, strict all-target Clippy and default workspace tests pass |
| Owned PostgreSQL | 76 storage tests (including the bounded benchmark), 13 runtime API tests, one model test, three CLI recovery/registry/install tests and source-owner rejection pass |
| Python controls | 38 development-helper tests and seven PoC-helper tests pass; operations helpers compile |
| Source inventory | Seven retained LocalNet PoC contracts match historical API inventory at the acknowledged snapshot boundary, across three historical pages and seven ingestion pages |
| Durable LocalNet feed | API restart/replay, durable ACK and reconnect pass; pruning six isolated outbox rows reports explicit authorized loss while canonical history remains queryable |
| Baseline recovery | A controlled quiescent fixture baseline and new subscription pass; concurrent atomic snapshot/feed handoff is **not** established |
| Owned backup/restore | 34 tables, 2,962 rows and three sequences match exactly in a separate empty restricted-owner database; current migrations/ACLs verified |
| Restored application | Worker resumes against existing read-only LocalNet source; API readiness, seven historical contracts and durable ACK resume without cursor pass |
| Restore guards | Same physical database is rejected, including normalized port spelling; populated target rejected before archive creation/restore |
| TLS | Real local CA/server controls: valid certificate succeeds; untrusted CA, wrong host and HTTPS-to-HTTP redirect do not forward credential-bearing HTTP requests |
| Availability/privacy | Source revocation during long poll fails closed; new event delivery/restart passes; stale worker blocks live readiness/feed while authorized historical reads remain available; health discloses no counters |
| Limits | Storage/API checks cover receipt pressure, monotonic late ACKs, expiry, hidden-only pruning, cross-principal/cursor denial, poll fairness and cancellation cleanup |
| Remote preparation | Placeholder TestNet configuration passes offline `ops check-config`; current-rights/package/source/destination `preflight` passed on LocalNet only; actual network/participant pairing not attested |
| Security | Fresh RustSec audit (1,561 advisories; 317 locked dependencies), dependency license/bans/source checks, actionlint and history/worktree secret scans pass; existing duplicate-version warnings remain |

Sanitized [LocalNet/restore evidence](../examples/operations/evidence.json) records
scope and limitations. Its LocalNet run identifies commit `b4d021f`; the subsequent
recovery run used the same implementation. Private runtime files, dumps, source
credentials and raw row fingerprints remain ignored under `.local`.
No DAR upload, ledger transaction or source-grant modification was performed.
Failures encountered here were our fixture startup/configuration or benchmark
setup, not evidence of an upstream defect.

### Bounded database measurements

Recorded debug-build results on x86_64/12 logical CPUs, PostgreSQL 17.12 loopback,
one reused connection, 256 contracts in four batches, one caller party and
25-row pages. Five warmups precede 50 measured iterations. Native SQL and JSON
decoding are included; HTTP/source-network authorization and concurrency are not.

| Operation | p50 | p95 | maximum |
| --- | --- | --- | --- |
| Canonical temporal first page | 35.440 ms | 37.555 ms | 39.156 ms |
| Unacknowledged delivery replay | 5.608 ms | 6.431 ms | 6.727 ms |

See [raw sanitized measurements](../examples/operations/benchmark.json) and
[reproduction](../examples/operations/README.md). These are a small local regression
baseline, not a production throughput, RPO or RTO claim. Live remote certificate,
source inventory, failure/recovery and capacity evidence await configured access;
follow [remote setup](guide/remote-source.md).

## M6 upgrade impact analysis — 2026-11-22

Shipped in alpha.5 after combined M5/M6 PR #30.
See the [guide](guide/upgrade-impact.md), [contract](design/impact.md) and
[PoC baseline/evidence](../examples/upgrade-impact/README.md).

| Check | Result |
| --- | --- |
| Dependency/report controls | 20 sync tests pass: unchanged/unselected changes, exact scale/optionality/removal/enum checks, arithmetic/filter/aggregate/key/join dependencies, deterministic redaction and bounded signature expansion |
| Policy review | Refreshing a target blocks unchanged incoming links with stale revision pins. Missing join targets block complete bundles. Unresolved target blockers propagate through incoming links, including cycles. Blocked plans retain useful reports and generate no artifacts |
| Evaluation | Exact before/after values, filtered/invalid samples, blocked candidates, duplicate/unknown fields and sample bounds tested; results omit payloads and values |
| Public CLI | Optional `sync_dar` test passes against the existing compiler-built PoC DAR: plan/evaluate/summary/bundle parity, overwrite refusal, redacted errors, no source connection |
| PoC sample | Full canonical PoC model has seven declared source reads; unchanged verified DAR needs no rebuild. A one-unit Int64 difference near its maximum is detected exactly |
| Regression checks | 119 default workspace Rust tests, 35 helper tests, seven PoC tests, formatting and strict all-target Clippy pass. Dependency/license, secret and workflow scans pass |

The pinned local audit and the online GitHub CI audit pass. All four PR/push
checks passed before merging M5/M6; main checks passed after merge.
M5's separate owned-database and retained-source results below remain the evidence
for its APIs. M6 adds no migration, dependency, source connection or ledger write.
The real-DAR CLI test is an unchanged-artifact baseline. Changed signatures and
blocked upgrades use synthetic LF graphs; no live package upgrade is claimed.

## M5 developer inspector — 2026-11-22

Shipped in alpha.5. The
[guide](guide/inspector.md), [contract](design/inspector.md) and
[example findings](../examples/inspector/README.md) describe the supported surface.
Migrations 001–010 and model/source fingerprints are unchanged.

| Check | Result |
| --- | --- |
| Transaction visibility | Owned PostgreSQL and mock-source REST/GraphQL agree for A/B/shared callers. Hidden/unknown transactions match; ordinals and counts include only independently visible indexed endpoints and source-event witnesses |
| Redaction and paging | No arguments/results, controllers, raw node bounds, witness lists, payloads or global checkpoints. Cross-transaction/caller/operation cursors fail; source outage, grant epochs and revocation stop delivery |
| Projection diagnosis | Exact native Numeric drift at `0.0000000001`, missing/unexpected rows, filtered/archived inputs and current model publication tested. Findings never repair rows; archived absence remains consistent even if a newer model cannot evaluate its old payload |
| Local commands | Matched/invalid payload findings contain no sentinel value/path; malformed shapes fail before I/O. Revoked operator authority yields no progress report and leaves the checkpoint unchanged |
| Retained source | Four M0 LocalNet SQL profiles reconcile inspector records with source metadata, including A/B subsets. Unindexed transients stay absent; source bounds use the restricted reader and no row-count scan |
| Interfaces | Offline/served diagnostic SDL signatures match; example TypeScript compiles with strict `tsc --noEmit` |

The final suite passed 106 default Rust tests, 67 owned-storage tests, eight runtime
API tests, five other database/CLI controls and four retained-source tests: 190
Rust tests total. The 35 development-helper and seven PoC-helper tests, formatting,
strict Clippy and security tools pass. Workspace package dry runs and archive
inspection are required final source checks; no upload is performed in this task.

Retained SQL checks use synthetic leases and an isolated owned destination, not
new live Ledger authorization. No source rows, parties, grants, DARs or ledger
transactions were changed. Failed operations have no durable historical trace;
`not_recorded` is reported rather than invented execution history. Global progress
is local operator-only. Production and rejected-command simulation are unverified.

The security gate discovered upstream [RUSTSEC-2026-0592](https://rustsec.org/advisories/RUSTSEC-2026-0592).
The existing transitive `url` dependency was updated to 2.5.9 with
`form_urlencoded` 1.2.3; no new dependency was added. No exploit through a
CantonGrid endpoint was demonstrated. This is an already patched upstream
advisory, not a new contribution candidate.

## M4 typed inner joins — 2026-11-22

Alpha.4 implements the [M4 contract](design/joins.md), with
[a runnable example](../examples/joins/README.md). No new dependency, destination
migration, ledger write or external deployment was needed.

| Check | Result |
| --- | --- |
| Declaration/type controls | Inner joins only; selected columns must exist, be unique and bounded. M3 definitions without join opt-in preserve their fingerprints |
| Owned PostgreSQL | 65 storage tests passed, including three M4 cases: independent A/B/shared input visibility, exact Int64/Numeric, bool/null, unselected columns absent, schema drift, replay, projection rollback and archive/recreate |
| Cursor state | Visible changes invalidate pages; hidden archives/creates do not invalidate a narrow caller's cursor. No global checkpoint is used as a hidden-update signal |
| Resource/lease denial | Hidden large values do not consume the caller's 2 MiB selected-result budget. Expiry while SQL waits on an owned relation lock returns no page |
| REST / GraphQL | Six runtime integration tests passed; join items/counts agree, transport cursors work, cross-root/caller/operation cursors fail, outer-join/column overrides are rejected and aliases share the two-resolver budget |
| Publication and authority | Target revision publication stops the pinned join; rollback restores it. Grant epochs, caller revocation and unhealthy/denied sources stop delivery |
| Generated interfaces | Served SDL matches offline signatures, including two namespaces with distinct target definitions. Example TypeScript passes strict `tsc --noEmit`; preview/SQL-plan commands pass |

The workspace suite passed 104 default Rust tests; model database and CLI
recovery/registry/install controls also passed. Strict Clippy, formatting,
35 development-helper and seven PoC-helper tests, dependency/advisory/license/
secret/workflow checks passed. Package builds and archive inspection are part of
the final source checks; no upload is performed.

These checks use real owned PostgreSQL with synthetic source leases and a mock
API source checker. They do not establish new LocalNet, production, global/outer
join, historical or multi-hop acceptance. Existing aggregate full-source guards
remain unchanged.

## M3 declared relationships — 2026-11-22

Included in alpha.4. The
[relationship contract](design/relationships.md) and
[example](../examples/relationships/README.md) describe the supported slice.
No dependency or destination migration was added.

| Check | Result |
| --- | --- |
| Model declarations | Names, duplicate policies, exact scalar/scale compatibility, unsupported optional/Boolean keys and immutable revision conflicts checked; absent declarations preserve legacy fingerprints |
| Owned PostgreSQL | 62 storage tests passed; new relationship case covers different templates, separate create batches, A/B/shared visibility, hidden fanout, exact large Numeric keys, archive/recreate, replay, stale bindings, cross-stream denial and projection rollback |
| REST / GraphQL | Five runtime integration tests passed with mock source authority; relationship counts/items agree, shared transport cursors work, cross-root/policy/caller cursors fail, hidden/unknown roots match, target publication/rollback and source policy revisions obey their pins |
| Current authority | Caller revocation, grant epochs and unhealthy/denied sources stop delivery; aliases share the existing two-resolver budget |
| Generated interfaces | Offline SDL signatures match served schema, example TypeScript passes strict `tsc --noEmit`, and ordinary/forged reference previews preserve the same declared key |

The workspace tests, strict Clippy, existing model registry/install/recovery
workflows, Python helpers, security checks and package dry run are part of the
final branch checks. Fixture source leases and the API's mock checker are not
live Canton evidence. No ledger writes, package uploads or grant changes were
needed for M3. Its current active projection queries do not imply historical
edges, proof of creation, joined payloads or transitive reachability.

## M2 direct lineage — 2026-11-22

Included in **0.4.0-alpha.4**, originally developed on `feat/privacy-safe-lineage`. Migration 010 adds private provenance/edge storage. Older
migration files remain unchanged. See the [design](design/lineage.md) and
[API guide](guide/lineage.md).

The [LocalNet evidence](../examples/exercises/lineage-evidence.json) records four
runs against existing generic and Outcomia packages, with real source authority
and independent application callers. Generic root edges were operator=3 and
A/B/shared=1 each; Outcomia root edges were operator=2 and A/B/shared=1 each.
REST and GraphQL agreed; ungranted/revoked callers were denied. Restricted
protocol observers could not query the unindexed market endpoint. These runs
used development source before the mechanical alpha.4 package-version change.
No DAR upload, new parties/users, grant widening, funding or trades occurred.

Proof tests cover complete nested subtrees, missing/unknown nodes, crossing
bounds, mixed transactions, duplicate node/creation identities and self-edges.
Owned PostgreSQL tests check visibility before counts/paging, endpoint and proof
policies, replay, SQL/model rollback and late lease expiry. API controls cover
root/direction/caller cursor binding, grant epochs, revocation, unavailable root
uniformity and unhealthy/denied source responses. Retained-source SQL tests use
synthetic leases and do not replace the real-authority LocalNet checks above.

Coverage is always partial recorded evidence and one hop. There is no inferred
ancestry from application fields, hidden-edge placeholder, transitive traversal,
declared relationship, join or receipt/lineage package handoff.

## M1 internal exercise receipts — 2026-11-21

The [receipt contract](design/exercise-ingestion.md) is implemented in the
0.4.0-alpha.3 release. Migration 009 was applied explicitly to the owned
PostgreSQL 17.12 destination; earlier migration checksums remain unchanged.

| Check | Result |
| --- | --- |
| `cargo test --workspace --locked` | 98 passed; environment-dependent tests ignored |
| Storage PostgreSQL suite, serial `--ignored` | 60 passed, including migration/ACL checks and model rollback |
| New receipt controls rerun after final integrity changes | Both pass, including late lease expiry and missing delivery marker |
| CLI API PostgreSQL suite, serial `--ignored` | 3 passed: REST/GraphQL scopes, revocation and outages |
| Other CI PostgreSQL controls | 5 passed: models, durable CLI, registry/install and source owner rejection |
| Read-only retained M0 source integration | Pass: four scopes, one-row snapshot pages, complete forward transactions and overlap |
| Visibility gate, owned PostgreSQL metadata probes | Pass: unknown offsets/witnesses reject; authorized redaction rejects; known outside-scope witnesses do not widen reads |
| Real LocalNet Rust worker | Four receipts/outbox entries, three consuming, two unindexed targets, one lifecycle archive; idle restart unchanged |
| Python helpers / existing PoC | 35 / 7 passed |
| Formatting, strict workspace Clippy, secret/log/release checks | Pass |
| Workspace package dry run and archive inspection | All ten `0.4.0-alpha.3` packages pass; no uploads |

The source integration test uses synthetic leases against real retained PQS;
it does not establish current grants. The separate worker check uses real Ledger
authorization and exporter health, reusing the approved fixture's pair/nested
choices without uploading DARs or provisioning more parties/users. Public
[worker evidence](../examples/exercises/worker-evidence.json) contains counts;
private payloads, identities and diagnostics stay under ignored `.local/`.

Source pruning/regression and transaction budget failures have synthetic controls;
no destructive pruning experiment ran. No public exercise API, package handoff
for receipt streams, interface-choice support, redaction, pre-snapshot history,
general lineage proof or remote production acceptance is claimed.

Release validation targets `0.4.0-alpha.3`. The first dry run at the reused
alpha.2 version resolved published dependencies without the new receipt APIs;
incrementing the workspace and internal dependency versions fixed verification.
No third-party dependency changed. Publication is a separate operator action.

## M0 exercise-source feasibility — 2026-11-21

The [source RFC](design/exercise-source.md#localnet-results) records the capability
matrix and decisions; [fixture evidence](../examples/exercises/evidence.json)
contains versions and nonsecret counts. The approved LocalNet experiment used
new synthetic parties/users and isolated PQS databases with restricted SQL readers.
Five transactions committed and two intended failures were rejected. Four scoped
views match Ledger Effects payloads, witnesses, transaction update IDs and node
bounds; one-row pages and overlapping replay pass after exporter restarts.

The two IDE-ledger Daml scripts, 35 helper tests and seven existing PoC tests pass.
Rust formatting, release metadata, secret scans and container log checks pass. Runtime Rust,
migrations and APIs are unchanged; a full Rust suite was not rerun for this scope.
SDK 3.7.1 selects damlc/daml-script 3.7.0. The helper uses a temporary source copy
to avoid an observed DPM 1.1.0 percent-encoded space in local DAR dependency paths;
this is a tooling workaround, not a classified upstream defect.

No general direct-parent proof, pruning, redaction, upgrade lineage or production
acceptance is claimed. Existing create/archive profiles were not converted to
tree mode. Raw payloads and identities remain private under ignored `.local/`.
Owned fixture services were stopped after verification; database volumes remain.

## Unreleased 0.4.0-alpha.2 offline metadata

Recorded 2026-11-21 without starting LocalNet or changing Ledger state. The CLI
extracted LF 2.1 metadata from existing local DARs; all package IDs, names and
versions matched independent `damlc inspect-dar --json` output (installed damlc
3.7.0, example manifests declare SDK 3.7.1).

| Artifact | Packages | Templates | Interfaces | Main package ID |
| --- | --- | --- | --- | --- |
| PoC | 31 | 1 | 0 | `730711263d9ca88f4ee397a7428ef775dff14ee144216dc08053f955f3cb357e` |
| Visibility | 31 | 1 | 0 | `05d72ea44339b0e3b89e195ccd3f72f67436219915ecc8d90e32b250fc285111` |
| Outcomia | 39 | 15 | 5 | `73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a` |

Counts include dependency packages. Compiler comparison covers package inventory,
not every field signature. Synthetic tests cover qualified/generic/recursive types,
Numeric scales, interface wire tags, archive corruption, missing references,
unsupported versions and resource limits. This does not verify ingestion upgrades
against a real Canton deployment, or generated models from DAR metadata (#24).

## Unreleased compound sorting and package upgrades

Recorded 2026-11-21 against the separately owned PostgreSQL 17.12 fixture and
mock source authorization. Compound ordering tests use equal prices with reversed
contract/FIFO IDs, mixed directions, nulls, one-row pages and shared REST/GraphQL
cursors. Existing single-sort requests and generated/served schema agreement pass.

Upgrade controls cover an atomic handoff, a 129-contract/three-page staged baseline,
reopened connections, read/write gating during publication, mixed-package forward
creates/archives, duplicate replay, stale worker/plan rejection, failed projections,
partial-publication cancellation and old-model rebuild rollback. Checkpoints and
historical batches are retained. Package-availability HTTP checks reject absent
packages under current read-only source grants.

These are implementation/owned-database controls. No new DAR was uploaded, no
Ledger transactions were submitted and no real Canton deployment-upgrade smoke
was performed for this change. The read-only runtime is still limited to the
verified LocalNet transport/PQS profile.

## 0.4.0-alpha.1 grouped aggregates

Recorded 2026-11-20 on the LocalNet profile above for source prerelease
`0.4.0-alpha.1`. Crates.io remains at 0.3.0.

- 67 credential-free Rust tests and 62 owned-PostgreSQL/mock-source checks passed.
  Coverage includes exact counts/sums, archive/replay, rollback, staged activation,
  caller/source revocation, cursor isolation and shared REST/GraphQL query limits.
- 30 Python helper controls, strict generated TypeScript compilation, formatting,
  Clippy and the security suite passed. All eight source archives passed the
  workspace publication dry run and package-content checks; no crates were uploaded.
- The read-only [LocalNet check](../examples/poc/aggregate-evidence.json) matched
  an independent authorized Ledger snapshot: seven existing PoC contracts in two
  groups. REST/GraphQL exact totals, pagination and narrow-scope denial passed;
  the original model revision was restored and no ledger transactions were sent.
- Outcomia status counts passed owned-PostgreSQL tests. This is not a live-ledger
  Outcomia aggregate acceptance run.

Aggregate reads require the complete configured source-party scope. Totals are
computed at read time over maintained contribution rows; partial-scope aggregates,
joins and independently maintained aggregate caches are not implemented.
See the [aggregate guide](guide/aggregates.md) for declarations and limits.

All eight `0.4.0-alpha.1` crates were published on 2026-11-20. Downloaded archives
contain nonempty READMEs and clean provenance at release commit `1f1e64b`.
Registry installation outside the checkout passed fixture init/validation/replay
and aggregate model validation, GraphQL generation and TypeScript generation.

## 0.3.0 release acceptance

Recorded 2026-11-19 on PostgreSQL **17.12**, using the same pinned Canton/PQS
profile above. Implementation and runner commit:
[`8ff178c`](https://github.com/kunal171/cantongrid/tree/8ff178c).

- Logical restores of the two PQS databases and owned destination matched all
  983 application-table row counts/hashes. Original PostgreSQL 14 volumes remain
  intact; no in-place major upgrade was attempted.
- 57 owned-PostgreSQL/mock-source checks passed on 17.12, alongside 64
  credential-free Rust checks, 29 Python/helper controls, formatting, Clippy and
  the security suite. Generated TypeScript compiled with strict checking.
- The full standalone PoC passed again: 35 accumulated controlled event identities,
  54 filtered page sequences, exact values, caller isolation/revocation, offline
  catch-up, revision rollback and exporter recovery.
- Outcomia passed 25 controlled lifecycle identities and 60 filtered page sequences
  on PostgreSQL 17.12, including private/shared scopes, caller revocation and
  optional-field promotion/rollback. No funded assets or trades were created.
- The [large rebuild](../examples/poc/rebuild-evidence.json) used 261 initial real
  contracts, fresh-process resume, actual source revocation, concurrent creates
  and an archive, stale-activation rejection and staged promotion/rollback.
  The final 264 test contracts were archived after checking all three caller
  scopes and exact Numeric/Int64 values. Source history remains partial.

Initial fixture setup resumed from its private journal after correcting a helper's
handling of an expected atomic-bind rejection. The rejected operation was correct;
this was a test-harness issue, not an engine or upstream failure.
The participant's upstream Quickstart database was not upgraded; compatibility
and restore evidence above covers only the PQS and CantonGrid databases.
Funded trading, joins/aggregates and streams remain outside this release.

## Alpha.5 local checks

Recorded 2026-11-19 during alpha.5 development, through code commit
[`0cca1ee`](https://github.com/kunal171/cantongrid/tree/0cca1ee):

| Check | Result |
| --- | --- |
| Credential-free Rust tests | 64 passed |
| Owned-PostgreSQL / mock-source HTTP checks | 57 passed |
| Development-helper / PoC helper tests | 23 / 4 passed |
| Formatting, Clippy, release metadata and log retention | Passed |
| Workspace publication dry run / source archive checks | Passed for all eight crates; no uploads |
| RustSec | No findings across 249 locked dependencies and 1,550 fetched advisories |
| Dependency licenses/bans/sources, actionlint, full-history and eligible-tree secret scans | Passed; no leaked secrets reported |

Dependency duplicate-version notices for `base64`, `syn` and `wasi` were not
vulnerability findings. These scans are point-in-time results, not an independent
penetration test. Source-dependent PQS tests and LocalNet scenarios were not
rerun for alpha.5.

### Staged rebuilds

Owned-PostgreSQL controls cover a 261-row model beyond the atomic limit, restart,
creates behind the cursor, archives before/after scanning, changes after readiness,
private staging, cancellation, aliases and stream isolation. SQL failures and
lease expiry after writes roll back rows and progress. Failed ingestion also
rolls back dirty IDs and checkpoints before a ready model can activate.

CLI checks use separate processes and a mock authorization/readiness server for
outage, revocation, promotion, cursor invalidation, cleanup and rollback.
Migration controls verify explicit 005→006 upgrade, unchanged earlier SQL and
rejection of checksum/privilege drift. The 0.3.0 checks above add large LocalNet acceptance.

### Security regressions

| Finding fixed in alpha.5 | Regression/control |
| --- | --- |
| Missing destination and projection column ACL checks | Real schema/table/sequence/column grants rejected; changes rolled back after each test |
| Admission after body extraction | A stalled HTTP body occupies a bounded slot and times out |
| Cancelled HTTP tasks released SQL capacity early | A running database task retains its permit after request cancellation |
| Canonical REST guard ended before encoding | Revocation during serialization returns only the denial body |
| Whole GraphQL response exceeded per-page cap | Aggregate and escaped JSON obey the complete 2 MiB limit |
| Canonical queries encoded all visible payloads for counts | PostgreSQL evaluation counter fails on the old query and proves page-only encoding |
| Broader PQS lease widened configured selection | Party-set mismatch rejected before SQL connection |
| Helper credential routing and partial/linked writes | No proxies/redirects; atomic 0600 replacement and link controls |

The ACL cases required operator SQL privilege changes; no unprivileged HTTP
cross-caller disclosure was demonstrated. Existing unintended grants must be
reviewed and revoked explicitly; startup validation does not revoke them.
See [security policy](security.md) for the maintained rules.

## Recorded LocalNet acceptance

| Release / date | Evidence and scope |
| --- | --- |
| 0.2.0-alpha.1 / 2026-11-17 | Eight controlled Outcomia create/archive identities with one-event pages and 17 worker invocations; Visibility counts A=2, B=2, AB=3; forged source tokens, caller isolation, revocation, exporter outage and recovery checked |
| 0.3.0-alpha.2 / 2026-11-18 | Two standalone PoC runs matched 7 then 13 accumulated event identities; generated TypeScript compiled and Chromium rendered the viewer |
| 0.3.0-alpha.4 / 2026-11-19 | Two PoC runs checked 54 filtered page sequences each, matching 23 then 29 cumulative event identities; exact values, discovery, shared transport cursors, revision rollback and exporter recovery passed |
| 0.3.0-alpha.4 / 2026-11-19 | Outcomia checked 60 page sequences, 25 controlled lifecycle events and seven active contracts; private/shared scopes, exact filters, optional-field promotion/rollback and caller revocation passed |

Artifacts: [standalone PoC](../examples/poc/evidence.json),
[Outcomia](../examples/fixtures/outcomia/model-query-evidence.json).
They retain the original development-commit metadata; those runs were subsequently
included in alpha.4. Chromium evidence is from alpha.2, not a repeated alpha.4 check.
The [release notes](releases.md) link historical results and earlier test totals.
No funded assets, trades or resolutions were created in the Outcomia run.

### Source observations

Schema041 archives have empty witnesses; explicit bounds are needed because the
observed `set_latest(NULL)` / `set_oldest(NULL)` calls returned NULL. A retained
80→943 interval reconciled 11 initial + 115 creates − 116 archives = 10 active
rows. This is historical reconciliation, not concurrent handoff proof.
Nodejumper SDK v0.3.2 (`5caaebc3eab380de827196663d4bb18b90ffd6f4`) decoded the
installed rows; no SDK dependency was added.

PQS cached rows survived source-rights revocation; readiness loss took 1–5 seconds
in two observations, not a guaranteed bound. An Outcomia repeat also stopped on
exporter token expiry; renewal restored export. This was fixture credential expiry.

### Outcomia artifact

The approved existing DAR used by the LocalNet check has SHA-256
`eef7340eba83f4f2af4865258c66c6461876bacb05b8e4dbc6c7ca74459f6f0e`
and main package
`73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a`.
The sibling source repository was not modified or claimed to have been rebuilt.

## Source defect

The [standalone PQS type-collision reproduction](https://github.com/kunal171/cantongrid/blob/main/repros/pqs-schema-type-collision/README.md)
contains the independent controls, exact versions and links to upstream issue
#240, PR #241 and CantonGrid issue #1. It separates the upstream SQL defect from
CantonGrid processing. Check upstream for current issue/PR status.

## Limits

Fixture and mock-source tests do not establish LocalNet behavior. No devnet,
remote/TLS, production load, complete pre-snapshot history, interface/reassignment,
join or durable-stream acceptance is claimed. Aggregate evidence is limited to the
alpha.1 slice above. See [next steps](README.md#next).
