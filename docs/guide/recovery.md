# Owned-destination backup and recovery

Back up before applying migration 011. Never migrate or restore into the PQS
source database. Retain the pre-upgrade backup until the new binary and all
consumers have been verified. An older binary rejects an unknown migration;
rollback requires the matching pre-upgrade database backup and configuration,
not deletion of migration rows or edits to applied SQL/checksums.

A database dump contains private payloads, parties, delivery receipts and consumer
progress. Keep archives owner-only and outside Git, artifacts and support reports.
Back up runtime configuration, reviewed grants and API/cursor signing keys through
your secret-management process separately. Keys are not stored in database dumps.
Restoring different keys or grants changes cursor bindings; ACK state alone cannot
make an old cursor valid.

## Local recovery acceptance

The source checkout includes a bounded helper for our owned loopback fixture.
Provision a **new empty database** owned by a restricted destination role. This
helper does not provision roles/databases, restore over existing tables, use
`--clean`, or delete a source. Pause workers and other destination writers first;
its exact before/after fingerprint comparison requires a quiescent database.

Set two distinct `CANTONGRID_*` connection variables privately, then:

```bash
cantongrid storage check --database-env CANTONGRID_DESTINATION_URL
python3 scripts/dev.py recovery verify \
  --source-env CANTONGRID_DESTINATION_URL \
  --restore-env CANTONGRID_RESTORE_URL \
  --output .local/recovery/new-private-run
```

The output directory must not exist. Connections require explicit numeric-loopback
host, port, user, password and database fields. The restore role must own the empty
database and have no elevated attributes or role memberships. The source and target
must be distinct databases. Use only trusted archives you created.

The helper checks the current source installation, creates a private custom-format
`pg_dump`, restores it transactionally with `pg_restore --no-owner`, and checks
restored migrations/ACLs. It compares exact PostgreSQL row representations in owned
schemas and sequence state; financial JSON does not pass through Python floats.
Fingerprinting is capped at 128 MiB with query deadlines; this is a local acceptance
helper, not a large-production backup service. The report contains counts and
booleans only. See [pg_dump](https://www.postgresql.org/docs/17/app-pgdump.html) and
[pg_restore](https://www.postgresql.org/docs/17/app-pgrestore.html).

## Resume deliberately

Before resuming against the restored database:

1. Restore the matching reviewed config, grants and independent signing keys.
2. Keep source identity, generation, template selection and source binding unchanged.
   Change only the destination connection to the restored database.
3. Run `storage check`, then `ops preflight` for the actual configured network.
4. Verify the source still retains the checkpoint interval; resume one worker and
   confirm its observation becomes fresh before exposing live delivery.
5. Verify scoped canonical/model reads and consumer replay/ACK under current grants.
   Resume without a cursor from the restored durable ACK. Effects committed after
   the backup can replay, so application deduplication remains necessary.

If source history was pruned or subscriber coverage was lost, stop and establish a
reviewed baseline. Do not jump the checkpoint or pretend an incomplete feed is
complete. This slice has no atomic snapshot/feed handoff during concurrent writes.
Current permission checks still apply after recovery. A successful local restore
does not prove remote disaster recovery, RPO, RTO or production capacity.
