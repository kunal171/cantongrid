# Verification

This page summarizes what has been tested, at which level, and where the evidence
lives. Results are dated records, not tests rerun when the page changes. The full
dated record for each milestone (test counts, commands and findings) is kept at
tag [`v0.5.0-alpha.2`](https://github.com/kunal171/cantongrid/blob/v0.5.0-alpha.2/docs/verification.md).

## Environment

| Component | Tested version |
| --- | --- |
| Rust / Python helpers | 1.100.0 / 3.11+ |
| CN Quickstart / Splice | `8b898ed3ccb15e243b22ee8d0ed3573fb91ee6b9` / 0.6.11 |
| Participant / PQS | Canton 3.5.7 / Scribe 3.5.4, schema041 |
| Database | PostgreSQL 17.12 (earlier alphas used 14.25) |
| Daml | SDK 3.7.1, LF 2.1 |

This is the tested LocalNet profile, not a general compatibility claim.

## Evidence levels

| Level | Meaning |
| --- | --- |
| **Offline** | Fixtures and unit tests; no database or source |
| **Owned PG** | Real owned PostgreSQL 17.12 with mock source authority |
| **Retained LocalNet** | Real LocalNet source, read-only, using contracts that already exist |
| **LocalNet writes** | Real LocalNet with ledger writes for the test |
| **Remote** | An authorized TestNet or production source (not done yet) |

## By feature

| Feature (release) | Highest level | Key results (date) |
| --- | --- | --- |
| Ingestion, caller isolation, revocation, exporter outage (0.2.0-alpha.1) | LocalNet writes | 8 Outcomia create/archive identities in one-event pages; Visibility A=2, B=2, AB=3; forged tokens rejected (2026-11-17) |
| Models, generated types, viewer (0.3.0-alpha.2) | LocalNet writes | PoC matched 7, then 13 event identities; TS compiled; Chromium rendered the viewer (2026-11-18) |
| Filtered and sorted model queries (0.3.0-alpha.4) | LocalNet writes | PoC: 54 page sequences per run; Outcomia: 60 sequences, 25 events, 7 active contracts, optional-field promotion and rollback (2026-11-19) |
| Staged rebuilds (0.3.0-alpha.5) | LocalNet writes | 261 contracts beyond the 128 atomic cap; resume, rights revocation, promotion and rollback |
| PostgreSQL 17 restore (0.3.0) | Retained LocalNet | 983 application-table row counts and hashes matched after logical restore (2026-11-19) |
| Aggregates (0.4.0-alpha.1) | Retained LocalNet | Exact totals and paging on both transports; narrow-scope denial |
| DAR metadata, compound sort, package upgrades (alpha.2) | Owned PG | 129 staged contracts, restart, stale-worker rejection, cancellation (2026-11-21) |
| Exercise source feasibility, M0 | LocalNet writes | 4 party views match Ledger Effects; 5 transactions (2026-11-21) |
| Exercise receipts, M1 (alpha.3) | Retained LocalNet | 60 storage + 3 API PostgreSQL tests; 4 receipts, 3 consuming exercises, no extra receipt on idle restart (2026-11-21) |
| Direct lineage, M2 (alpha.4) | Retained LocalNet | Generic 3 operator edges (1/1/1 scoped); Outcomia `Protocol_CreateBinary` 2 (1/1/1) (2026-11-22) |
| Declared relationships, M3 (alpha.4) | Owned PG | 62 storage + 5 API tests: A/B/shared, hidden fanout, large Numeric keys (2026-11-22) |
| Typed inner joins, M4 (alpha.4) | Owned PG | 65 storage + 6 API tests; hidden values don't use the 2 MiB budget (2026-11-22) |
| Inspector, M5 (alpha.5) | Retained LocalNet | 4 M0 SQL profiles reconcile with inspector records (2026-11-22) |
| Upgrade impact, M6 (alpha.5) | Offline | DAR fixtures, pin drift, blocked bundles (2026-11-22) |
| Temporal reads and durable delivery, M7–M9 (alpha.6) | Retained LocalNet | 7 PoC contracts match the as-of inventory across 3 pages; restore guards (2026-11-23) |
| Atomic bootstrap (alpha.7, then 0.4.0) | Retained LocalNet | 7 PoC contracts across 3 baseline pages, API restart between pages; TS example tests (2026-11-23) |
| Reconciliation policies, preflight, indexed checks (0.5.0-alpha.1) | Owned PG | 92 storage + 15 HTTP tests; 11 policy controls (2026-11-23) |
| Reconciliation API (0.5.0-alpha.2) | Retained LocalNet | 7 PoC contracts; Ledger and PQS inventories match; violation, denial, revocation, restart (2026-11-23) |

**Release checks.** Each release also passes these:
- **Lint:** formatting and strict Clippy.
- **Security:** RustSec, plus the dependency, license and ban policy (no findings
  across 249 locked dependencies at alpha.5).
- **Hygiene:** actionlint and secret scans of history and tree.
- **Packaging:** packaging dry runs for all ten crates.

**Evidence files:**
- **PoC:** [`examples/poc/evidence.json`](../examples/poc/evidence.json) and
  `rebuild-evidence.json`.
- **Outcomia:**
  [`examples/fixtures/outcomia/model-query-evidence.json`](../examples/fixtures/outcomia/model-query-evidence.json).
- **Exercises:** [`examples/exercises/`](../examples/exercises/README.md).
- **Reconciliation:** [`examples/reconciliation/localnet-evidence.json`](../examples/reconciliation/localnet-evidence.json).
- **Benchmark:** [`examples/operations/benchmark.json`](../examples/operations/benchmark.json).

None of them contain party IDs, contract IDs or credentials.

## Benchmark

| Operation | p50 | p95 | Max |
| --- | --- | --- | --- |
| Temporal first page | 35.4 ms | 37.6 ms | 39.2 ms |
| Unacknowledged delivery replay | 5.6 ms | 6.4 ms | 6.7 ms |

**Setup.**
- **Build:** debug, on x86_64 with 12 CPUs, against loopback PostgreSQL 17.12.
- **Data:** 256 contracts, read in 25-row pages.
- **Runs:** 50 iterations after 5 warmups.

This is a regression baseline, not a throughput, RPO or RTO claim.

## Security regressions fixed in alpha.5

| Finding | Control |
| --- | --- |
| Missing destination and projection column ACL checks | Real grants rejected (needed operator SQL privileges; no HTTP cross-caller leak shown) |
| Admission after body extraction | A stalled body holds a bounded slot and times out |
| Cancelled tasks released SQL capacity early | A running task keeps its permit |
| REST guard ended before encoding | Revocation during serialization returns only the denial |
| GraphQL response over the per-page cap | Whole response capped at 2 MiB |
| Counts encoded every visible payload | Encoding is page-only |
| A broader PQS lease widened the selection | Party-set mismatch rejected before connecting |
| Helper credential routing and partial writes | No proxies or redirects; atomic 0600 writes |

## Source observations

- **Archive witnesses:** schema041 archives have empty witnesses, so archives need
  a create that was already observed.
- **Bounds:** `set_latest(NULL)` returned NULL, so explicit bounds are needed.
- **Reconciliation:** a retained 80→943 interval reconciled as 11 + 115 creates −
  116 archives = 10 active.
- **Cached rows:** PQS kept cached rows after rights were revoked. Readiness loss
  took 1–5 s in the two observations, which isn't a guaranteed bound.
- **Outcomia DAR:** SHA-256 `eef7340e…9f6f0e`, main package `73c2c43b…ba114a`.
- **Upstream defect:** the PQS type-collision defect is reproduced in
  [`repros/pqs-schema-type-collision`](https://github.com/kunal171/cantongrid/blob/main/repros/pqs-schema-type-collision/README.md)
  (upstream #240 / PR #241).

## Not verified

- **Remote:** remote networks (TestNet, Devnet, MainNet) and remote TLS.
- **Scale:** production load.
- **History:** complete history from before the snapshot.
- **Unsupported shapes:** interfaces and reassignments.
- **Composition on LocalNet:** joins and relationships on LocalNet.
