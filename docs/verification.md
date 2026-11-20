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
