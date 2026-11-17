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
