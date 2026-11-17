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
