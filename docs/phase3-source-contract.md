# Phase 3 source contract

Current status: the supported Phase 3 LocalNet profile is delivered; see
[acceptance evidence](phase3-verification.md). The original design/research
baseline below is retained, with the resolved implementation notes at the end.
Phase 4 model work is tracked separately in [its contract](phase4-models.md).

Status: source-reviewed and read-only LocalNet-tested, 2026-11-17. This records
verified interfaces before live-adapter work. Private copying, current-grant
verification and authenticated serving are not enabled by these probes.

## Baseline

The runtime now additionally includes `ledger`, `runtime` and the in-progress
`models` compiler; the research baseline below predates those integrations.
[PR #3](https://github.com/kunal171/cantongrid/pull/3) merged owned PostgreSQL
reference persistence after both CI runs passed. The supplied v0.3 handoff's
statement that no owned destination exists is historical.

Installed source: App Provider Canton 3.5.7, Scribe 3.5.4, schema 041,
PostgreSQL 14.25. The provider reader is nonowner/SELECT-only, with no role
membership or schema/database/temp/write privileges. Probes recheck this in
read-only repeatable-read transactions; source SQL is never a migration target.

## Verified interfaces

| Interface | Contract and restriction |
| --- | --- |
| `active(qname, offset)` | Snapshot at a participant-local boundary. A later archive offset on a returned row does not make it inactive at the historical boundary. |
| `creates(qname, from, to)` | Inclusive retained interval; resume after B with `created_at_offset > B`. |
| `archives(qname, from, to)` | Inclusive retained interval; resume with `archived_at_offset > B`. Schema 041 deliberately returns empty witnesses. |
| `oldest_offset`, `latest_offset`, `pruned_offset` | Resolve inside the same snapshot. Positions can be sparse; subtraction is not an event count/global height. |
| `set_latest(NULL)`, `set_oldest(NULL)` | Return NULL here; new code passes explicit arguments without shared temporal session settings. |
| Identity | Preserve upstream event/contract IDs, exact template/package, projection payload type and source/network/scope generation. |
| Payload | Preserve configured Numeric/Int64 strings; unverified metadata does not justify guessing a typed value. |

Installed signatures/columns were inspected locally. The upstream controls are
[schema migration sources](https://github.com/digital-asset/participant-query-store/tree/v3.5.4/modules/postgres/document/resources/db/migration).
A different source version requires new compatibility evidence.

## Retained-window reconciliation

`source verify-window --database-env CANTONGRID_PQS_URL --after 80` checks:

```text
active(B1) = (active(B0) union creates in (B0, B1]) minus archives in (B0, B1]
```

Only aggregate identity-set counts leave the source. The key distinguishes exact
creation packages and template/interface projections. At B0=80/B1=943 the probe
observed baseline 11, creates 115, archives 116 and active 10, with zero missing or
unexpected rows. The empty interval also reconciles. No payloads/party IDs were
copied or printed and no ledger transaction was submitted.

This is retained historical consistency, not a controlled concurrent publication,
pagination or reconnect proof. The report keeps `concurrent_handoff_verified=false`
and coverage `retained_interval_only`. Invalid/regressed/unavailable/pruned bounds
fail. An actual page cursor needs a verified same-offset tie breaker; offset-only
advancement can miss additional events at the same participant position.

## Nodejumper SDK evaluation and decision

Reviewed [canton-rust-sdk v0.3.2](https://github.com/nodejumper-org/canton-rust-sdk/tree/v0.3.2),
commit `5caaebc3eab380de827196663d4bb18b90ffd6f4` (Apache-2.0). The author reports
Scribe 3.5.4/Canton 3.5.7 verification. An independent ignored evaluation workspace
also tested its row mapping against our installed, restricted read-only source.

`Contract<serde_json::Value>::from_row` decoded 3 create rows, 3 archive rows and all
11 offset-80 active rows. Eight active rows had later archive offsets and remained
correctly active under `was_active_at(80)`. Archive witnesses remained empty.
No SDK writer/admin suites were run; no SDK runtime dependency was added.

Decision: retain a thin version-checked transaction adapter over the existing
PostgreSQL driver for the first generic source contract. The SDK is compatible
with these rows; it is not being rejected for a fabricated version mismatch.
Specific requirements not provided by its public typed client:

- The typed row omits upstream create/archive event IDs, template fully qualified
  name and projection payload type needed by our canonical provenance.
- Queries return complete vectors and reconnect/retry individually. There is no
  public transaction/portal interface pinning our complete paged read to one
  checked transaction. Raw BEGIN through a reconnecting client cannot establish it.
- Generic selections use exact runtime template metadata rather than requiring
  one compiled application type per template.

Reuse remains an option for typed/codegen integration after separate review. Do not
build a replacement general SDK, ledger writer or application code generator.

## Live authorization gates

Empty archive witnesses require a verified lifecycle policy. Preserve the raw
empty evidence; investigate whether an archive can depend on a previously observed
contract plus current grants without pretending inherited policy is an upstream
witness. Until that semantic is demonstrated, restrict the supported subset and
label history partial/unsupported.

Still open: current source-user/grant renewal and revocation, historical access and
retention policy, concurrent handoff/pagination/disconnect/pruning tests, dynamic
Daml metadata normalization, real application identities and scoped query cursors.
Actual Outcomia data is also required: DAR validation is not live indexing.
Unavailable data must remain incomplete. This document authorizes no new parties,
DAR uploads or ledger transactions.

## Proposed API dependencies

A concrete serving plan would use axum 0.8.9 (HTTP), async-graphql 7.2.1 (generic
GraphQL), reqwest 0.13.5 (bounded current-grant probes), jsonwebtoken 12 (validated
application bearer tokens), and direct tokio (already transitive). Use minimum
features and audit the resolved lockfile. Source tokens and API signing keys stay
separate. All five dependencies were subsequently approved and adopted for Phase 3. The v0.3 spec
requires maintainer approval before new third-party hard dependencies.

## Machine-readable coverage

The inspector now exposes `coverage.status` (`unknown` or `partial_history`) and
stable reason codes for unavailable archive evidence, unverified current source
authorization, concurrent handoff, source freshness, absent snapshots and pruning.
It never upgrades SQL availability or nonempty results to complete history/current
permission. Pruned/unavailable update windows return a distinct gap error, including
the pruning boundary until its retention semantics are verified. No automatic
checkpoint skipping, data deletion or re-snapshot is triggered by that error.

## Delivered LocalNet subset

The live partial-history policy, current source/caller checks, forward paging and
both API transports are now integrated. Controlled Outcomia publication during a
paged snapshot matched all eight expected lifecycle contract identities, including
same-offset events and restart. Original event offsets are retained separately
from delivery cursors; raw operator witness sets/PKs are not public API fields.
Actual Outcomia and unrelated Visibility both passed the same engine. Earlier
probe-only caveats describe their historical scope; current acceptance and limits
are authoritative in [Phase 3 verification](phase3-verification.md) and
[the runtime guide](live-runtime.md). Full historical completeness, interface/
reassignment support, generated type normalization and remote source integration
remain unsupported; schema041 availability is never a complete global ledger.
