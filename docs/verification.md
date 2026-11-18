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
Live model storage, lifecycle integration and generated APIs remain untested
because they are not implemented.

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
complete pre-snapshot history, automatic erasure, live typed models, durable
streams, devnet or production acceptance is claimed. Fixture runs are synthetic.
For new failures, separate our configuration/processing from documented source
behavior, then reproduce any suspected upstream defect with nonsecret controls.
