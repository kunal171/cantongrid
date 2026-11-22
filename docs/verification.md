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

## M2 direct lineage — 2026-11-22

Implemented on `feat/privacy-safe-lineage`, preparing **0.4.0-alpha.4**; not a
published release. Migration 010 adds private provenance/edge storage. Older
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
