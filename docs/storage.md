# Durable ingestion milestone

Current status: the supported Phase 3 LocalNet profile is delivered; see
[acceptance evidence](phase3-verification.md). The original design/research
baseline below is retained, with the resolved implementation notes at the end.
Phase 4 model work is tracked separately in [its contract](phase4-models.md).

Status: implemented and tested as a **bounded local reference feature** on
`feat/pqs-ingestion-storage`: owned PostgreSQL state/history/checkpoint/outbox,
restart/replay and writer exclusion; durable fixture CLI commands; and a read-only
PQS inspector. [Commands](cli.md) and [verification](verification.md) record the
actual environment tested. This does not complete Phase 3's live connector/APIs.

The store preserves original evidence in versioned envelopes and binds a generation
to its exact canonical plan definition. Trusted visibility policies/grants are
recomputed, never deserialized from a journal. Fixture caller grants remain
simulated, so these commands must only be used with synthetic fixture input.

## Live source gate discovered

Read-only inspection of the installed Scribe 3.5.4/schema 041 confirms the public
`creates(text,bigint,bigint)` / `archives(text,bigint,bigint)` signatures. The archive
function deliberately returns an empty witness array. This behavior is explicit
in the upstream [V033 migration](https://github.com/digital-asset/participant-query-store/blob/main/modules/postgres/document/resources/db/migration/V033__Implement_contracts_divulgence.sql)
and current functions. It is documented source behavior, not a newly demonstrated
upstream bug. Missing archival evidence must not be substituted with invented
parties, signatories, create witnesses or an unfiltered fallback.

The Rust source inspector reports this restriction without copying private
payloads or claiming source readiness/authorization. Live archive ingestion,
current source-grant retrieval, historical retention policy, snapshot/update
handoff and actual Daml metadata normalization remain implementation gates.
Durable fixture success will not mark them resolved.

## Owned local database

`python3 scripts/dev.py storage up` starts a separate PostgreSQL 14.25 fixture on
loopback 5442. It has its own volume/network and a restricted `cantongrid_owner`
role; the PQS source on 5441 is unchanged. `storage stop` retains the volume.
Bootstrap SQL is operator-side fixture setup, not a source migration.

The Rust connection boundary accepts one explicit numeric loopback host and
user/database. Remote hosts, multi-host/DNS overrides, arbitrary session options
and required TLS are rejected; production transport support remains unimplemented.
Connection/SQL errors omit driver diagnostics. Credentials come from dedicated
`CANTONGRID_*` environment variables rather than committed project definitions.

Destination initialization verifies database ownership, restricted role privileges,
absence of PQS's public `active` function and schema ownership. Schema version 1
and the immutable migration checksum are recorded under a transaction-level
migration lock. Repeat initialization verifies them; it does not adopt or rewrite
an unrelated/incompatible schema. PostgreSQL tests confirmed repeat initialization
and checksum-drift rejection. PostgreSQL-dependent tests are explicitly ignored
by default and must be run against the owned fixture.

## Durable reference engine

The Rust store commits a source batch's canonical contracts, lifecycle observations,
original evidence envelope, outbox references, checkpoint and revision together.
A transaction-level advisory lock excludes another writer for that source context.
[Transaction advisory locks](https://www.postgresql.org/docs/14/explicit-locking.html#ADVISORY-LOCKS)
are database-local and released on transaction/connection termination; no
application process lease needs to be guessed. Locks coordinate cooperating engine
workers; the trusted database owner can still modify SQL directly.

Recovery replays committed, versioned envelopes in contiguous destination revision
order. Source positions remain opaque (for example `fixture-z` can precede
`fixture-a`). The checkpoint must equal the last committed position. Exact JSONB
payloads retain decimal/Int64 strings. Archive visibility stores both the original
create and archive witness clauses; callers require each clause, not their union.
A changed source selection/party set/model schema in the same generation is rejected.
A different generation starts a separate stream and requires rebuilding.

A duplicate committed delivery performs no additional state/outbox writes and does
not regress the checkpoint. Unknown commit outcomes require reconnecting and
replaying the same batch. Outbox identity sequences can contain rollback gaps;
they are not ledger positions or public authorization-bearing stream cursors.

This slice replays a bounded journal rather than claiming a scalable storage
implementation: maximum 256 batches, 1000 events/1 MiB per batch and 16 MiB of
journal JSONB text per source generation. Limits fail without advancing progress.
No retention/deletion of an active stream is automatic. Recovery detects envelope,
revision and checkpoint inconsistencies; it does not repair operator SQL changes.

Real PostgreSQL checks passed for reopen/replay, exact values, A/B/AB read policies,
lease invalidation, writer exclusion, source-definition drift, invalid/conflicting
batches, size rejection and corrupted checkpoint denial. A test-injected division
by zero after all staged writes exercised an actual SQL transaction abort: canonical
state, history, outbox and progress were all unchanged after reconnect.

## PQS read-only inspector

`cantongrid-pqs` now inspects schema 041 using a restricted nonowner reader and a
repeatable-read/read-only transaction. It checks role/schema/table privileges and
rejects security-definer functions and unexpected schema versions. It resolves
and passes explicit temporal bounds to public PQS functions, reports row counts
and coverage limits, and copies no payloads or party identifiers to the destination.
Its live LocalNet check passed under the existing SELECT-only provider reader.

The report always leaves private lifecycle ingestion disabled. No rows or zero
archive rows cannot prove a missing capability exists. The SQL inspector does not
check source credential expiry, source readiness, or current ledger/user grants;
its result cannot authorize ingestion or application access. No second connector,
upstream mutation or unfiltered fallback was introduced.

## Authorized SQL query foundation

`Store::read_page` now provides one backend read path for active contracts and
lifecycle history. It checks a trusted caller/source lease intersection before
SQL and again before returning data. Scope/generation or plan-definition mismatch
fails closed. It does not deserialize caller grants from request filters.

Visibility predicates run **before** total counts and keyset pagination in a
read-only repeatable-read transaction. History requires every stored dependency
clause; empty/malformed evidence cannot become public. Page size is 1–100. Reads
use canonical SQL state rather than replaying the entire journal for each page.
Original identity sort order is a paging contract, not ledger chronological order.

PostgreSQL tests cover A/B/AB/no-party totals, one-row pages, duplicate-free paging,
revocation, conjunctive archive dependencies and empty-evidence denial. Debug output
redacts page data. The raw backend `next_after` key is **not** a public API cursor:
REST/GraphQL must wrap it with principal/source/generation/expiry integrity checks.
Real caller authentication and current source-grant retrieval still need the
separate provider/transport implementation. These tests use synthetic fixture grants.

## Live profile added in 0.2.0-alpha.1

The fixture reference retains its bounded full-journal replay. Live indexing uses
SQL-native row lifecycle checks and a guarded expected checkpoint, so continuous
operation does not hit the fixture's 256-batch/16-MiB journal replay cap. Deliveries
are bounded; state, events, source cursor and outbox still share one transaction.
A stale concurrent worker cannot rewind progress, failed writes roll back the
whole page, and ambiguous commit acknowledgement is resolved by reopening the
acknowledged tail. Journal/history disk usage grows with retained data; automatic
retention/erasure is not implemented.

New immutable owned migration 002 adds observation timestamps and its own checksum
record. Run explicit initialization to migrate an existing owned destination;
opening alone never migrates. No upstream source schema is modified. Read pages
expose observation age and first/complete source boundaries to the authenticated
transports. See [runtime configuration and limits](live-runtime.md).
