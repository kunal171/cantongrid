# PQS compatibility evidence

Verified locally on 2026-11-17. These findings support a first local connector
candidate, not a completed CantonGrid connector or release acceptance.

## Tested profile

| Component | Observed version / scope |
| --- | --- |
| Participant | LocalNet App Provider, Canton 3.5.7. |
| Source exporter | Scribe 3.5.4, built against Daml SDK 3.5.2. |
| SQL document schema | 041. Cached PQS 3.7.0/schema 043 was not used. |
| Database | Separate PostgreSQL 14 fixture on host loopback 5441. |
| Ledger principal | Dedicated user with exactly one CanReadAs party grant; no ActAs/admin grants. |
| Records | Existing authorized Splice development contracts; no Outcomia deployment. |

See [setup and reproduction commands](pqs-local.md). Source credential, actual
party IDs and private payloads stay outside committed evidence.

## Observed SQL behavior

Introspection and actual queries verified `active`, `creates`, `archives`,
`exercises`, `latest_offset`, `oldest_offset`, `set_latest`, `set_oldest`,
`pruned_offset` and `validate_offset_exists` in the installed schema. The probe
uses only the public SQL functions for contract data, never private storage tables.

In schema 041, `set_latest(NULL)` and `set_oldest(NULL)` return their supplied NULL
argument. They do not return the resolved boundary described by current docs.
The working probe first resolves `latest_offset()` / `oldest_offset()`, then pins
those numeric bounds in a dedicated repeatable-read, read-only connection.
Future pooled Rust clients must set/reset temporal context explicitly or use
explicit offset arguments; session settings must not leak between requests.

A first observed snapshot at offset 80 contained 11 active rows, 17 create rows
and 6 archive rows. All create rows included the configured party in their
witness evidence, and there were no duplicate create event/template/payload-type
identities. The exercise query returned zero rows; exercise content ingestion
has not been demonstrated. Counts may include interface projections and are not
universal counts of unique business contracts.

## Recovery and source health

Stopping only the PQS process left its database intact, and the probe correctly
failed instead of treating stored rows as a healthy source. Recreating PQS with a
fresh token reused the stored progress. Newer source updates were observed at
offset 138, with 22 creates and 12 archives. Create identities remained unique.
The explicitly pinned offset-80 result still exactly matched its pre-restart
snapshot even after those newer updates.

`/readyz` returned status ok with gRPC, JDBC pool and stream flags true. This is a
connection check, not proof of instant source authorization revocation or a
complete history. Source offsets are participant-local and may have gaps.
These tests verify the upstream PQS fixture's recovery, not a future Rust
indexer's destination checkpoints or outbox atomicity.

## Remaining gates

- Verify authorized Outcomia contract creation/lifecycle reads and an unrelated
  Daml example; DAR validation alone does not establish those flows.
- Verify A/B/AB visibility, joins/aggregates, revocation, token expiry, historical
  access policy and removal of materialized private data.
- Verify exact numeric round trips, full schema extraction, exercises,
  reassignment/incomplete events, pruning gaps and concurrent snapshot handoff.
- Design a least-privilege SQL reader role and a separately owned CantonGrid
  destination. The fixture's local SQL owner is not an application authorization model.
- Build and test the Rust normalization, processing, checkpoint and API boundaries.

PQS/Scribe 3.5.4 is therefore a verified local source candidate for active and
create/archive queries. Its version-specific behavior must be explicit; neither
newer versions nor broad production guarantees follow from this test.
