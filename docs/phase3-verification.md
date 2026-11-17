# Phase 3 acceptance — LocalNet contracts profile

Verified locally on 2026-11-17 for release candidate `0.2.0-alpha.1`.
The supported Phase 3 implementation is complete at the explicit LocalNet profile
in [the runtime guide](live-runtime.md). This is not a production certification,
a global-history claim or a published release. Remote CI/release review remains
required before publication.

## Environment and permissions

- Rust/Cargo 1.100; Python 3.11+ development helpers.
- LocalNet App Provider Canton 3.5.7; Scribe 3.5.4, schema041; PostgreSQL 14.25.
- Separate restricted SQL readers for provider and AB Visibility sources; no
  source ownership, role membership, schema/temp/table-write or source migrations.
- Separate CantonGrid-owned destination, role, port, volume and immutable
  migrations 001/002. Explicit initialization applies 002 and checks both hashes.
- Existing A/B test parties and read-only source scopes; organization API tokens
  are separate from source credentials.
- The maintainer explicitly approved local-only Outcomia DAR upload/test lifecycle.
  No devnet, paid infrastructure, funded assets, collateral transfers, orders or
  trades were created. The sibling Outcomia repository was read, not modified.

## Acceptance matrix

| Requirement | Evidence | Result and limit |
| --- | --- | --- |
| Authorized source | Existing one-party source user accepted; forged same-subject token rejected by the actual Ledger. Real AB PQS read through its separate SELECT-only role. | Passed for exact CanReadAs/default identity provider/loopback. |
| Snapshot → updates | Outcomia protocol observed in PQS before snapshot; market changes submitted while snapshot paging could continue. All eight transaction-produced create/archive contract identities appeared in the index with single-event pages. | Passed for schema041 template profile; history remains snapshot-forward partial. |
| Atomic destination | Owned DB tests cover checkpoints/outbox, actual SQL rollback, staged-event lifecycle failure, migration drift and writer exclusion. Live worker uses expected-checkpoint comparison. | Passed; a stale worker cannot rewind another worker's progress. |
| Replay/restart | Duplicate exact live delivery returns no new effects; CLI restart/replay tests pass; Outcomia demonstration uses 17 worker invocations and one-event pages. Live test passes beyond 256 batches. | Passed; live reads do not replay the full journal. |
| Privacy | A/B/AB record identities and totals checked through REST/GraphQL; cursor tampering, principal/kind/epoch reuse, issuer/audience/signature and expiry controls. Local caller revocation denies an existing principal immediately on the next request. | Passed for single-organization scoped grants. Source staleness is bounded by a 15-second lease; not instantaneous remote revocation. |
| Source outage | Actual AB exporter stopped while SQL rows remained; worker denied advancement and both APIs returned 503. Exporter restarted and the API recovered. | Passed; no stale-cache fallback on unknown source authorization/readiness. |
| Genericity | Actual Outcomia Protocol/BinaryMarket and unrelated Visibility:Record use exact template configurations through the same reader/store/APIs. | Passed; no Outcomia production branch, custom SQL or resolver. |
| Exactness | Visibility amount `12345678901234567890.1234567891`/Int64 `9223372036854775807`; Outcomia price tick `0.0000000001` and maximum Int64 lot survive actual PQS → PostgreSQL → API. | Passed as explicit raw JSON strings; no inferred typed arithmetic. |
| Provenance/coverage | Original event offsets stored separately from page keys; public history marks snapshot observations and prior-create archive policy. Partial/unknown coverage, history start, watermark and observation age returned. Pruned/regressed cursors fail. | Passed; raw source witness sets and internal PK/cursor envelopes are not public fields. |
| Regression | Fixture CLI, durable reference store, strict configuration, authorization core, source probes and security/release tooling. | Passed at the supported profile. |

## Executed checks

- **47 credential-free Rust tests** passed.
- **14 environment-dependent Rust tests** passed: 8 owned storage, 3 source/schema/
  retained-window, 2 CLI workflow/source-authentication and 1 transport privacy
  integration. **61 Rust tests total**; environment-dependent tests were explicitly
  run rather than counted as passing while ignored.
- **13 Python helper tests**, compilation, formatting and workspace Clippy with
  warnings denied passed.
- Actual read-only Visibility pipeline passed with authorized active counts
  **A=2, B=2, AB=3**, exact values, scoped identities/pages, worker restarts,
  local caller revocation and source stop/start denial/recovery.
- Actual explicitly authorized Outcomia test passed with its current Protocol and
  Closed BinaryMarket, all **8 controlled lifecycle events**, single-event paging,
  worker restarts, original source provenance and both API transports.
- RustSec, dependency source/license policy, full-history/eligible-tree secret
  scanning, actionlint, release metadata and Compose log-retention checks passed.
  The approved GraphQL/HTTP/JWT dependencies require BSD-2/BSD-3 notices; those
  standard licenses were explicitly added to policy. No advisory/secret ignores.
- Builds reused the existing private temporary Cargo target. All 15 development
  containers retain log rotation; database volumes were preserved. Disk/service
  health was checked after testing.

Credential-free and owned-database CI commands are in the workflow and
[contributor guide](../CONTRIBUTING.md). LocalNet commands are in
[the runtime guide](live-runtime.md). Ignored owner-only reports live under
`.local/pipeline/` and `.local/outcomia-pipeline/`; no token/party/private payload
was committed or printed as verification output.

## Actual Outcomia artifact and triage

The uploaded existing DAR SHA-256 was
`eef7340eba83f4f2af4865258c66c6461876bacb05b8e4dbc6c7ca74459f6f0e`,
main package `73c2c43b962bb68d033a62182bbb76ed27b50375a8b6ded3314220e5d1ba114a`.
It is the previously validated artifact, not an assertion that a sibling source
checkout was rebuilt. The authorized helper pins that package preference for
commands and exact creation-package selections for ingestion.

Initial command submission used a package-ID identifier where the installed API
required a package-name identifier; the example now uses the verified package
name plus exact package-ID selection preference. An example API issuer/audience
mismatch and page limit mismatch were also corrected. These were our example
configuration errors, not evidence for an upstream bug or public issue.
Repeated explicit test runs retain isolated test protocols/market revisions;
volumes/application data are never reset to make a probe pass.

## Deferred scope

No remote/TLS transport, alternate identity provider, interface projections,
divulged-only/reassignment lifecycle, complete pre-snapshot history, automated
retention/erasure, dynamic generated Daml types/business models, streams, Scan,
devnet or production readiness is claimed. Growing retained journals/data require
normal operator capacity planning. Phase 4 introduces deterministic typed models;
Phase 5 introduces authenticated durable streams and further operations work.
