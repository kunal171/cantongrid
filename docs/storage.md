# Durable ingestion milestone

Status: in development on `feat/pqs-ingestion-storage`. The first increment adds
versioned batch envelopes and canonical plan definitions. They preserve original
source evidence; trusted policies/grants are never deserialized from a journal.

Next: a separate owned PostgreSQL destination, atomic state/history/checkpoint/
outbox commits, restart replay, writer exclusion and fixture CLI integration.
The initial implementation remains a bounded reference engine, not a throughput
claim. Simulated fixture grants do not authenticate application callers.

## Live source gate discovered

Read-only inspection of the installed Scribe 3.5.4/schema 041 confirms the public
`creates(text,bigint,bigint)` / `archives(text,bigint,bigint)` signatures. The archive
function deliberately returns an empty witness array. This behavior is explicit
in the upstream [V033 migration](https://github.com/digital-asset/participant-query-store/blob/main/modules/postgres/document/resources/db/migration/V033__Implement_contracts_divulgence.sql)
and current functions. It is documented source behavior, not a newly demonstrated
upstream bug. Missing archival evidence must not be substituted with invented
parties, signatories, create witnesses or an unfiltered fallback.

A Rust source inspector will report this restriction without copying private
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
Locks are database-local and released on transaction/connection termination; no
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
