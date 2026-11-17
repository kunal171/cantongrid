# PQS initial migration: type collision across schemas

Confirmed locally on 2026-11-17 with PostgreSQL 14.25. This reproduction needs
Docker and Python only. It does not need Canton, Daml contracts, CantonGrid
processing, ledger credentials or an existing database.

```bash
python3 repros/pqs-schema-type-collision/run.py
```

The script extracts the unmodified V001 migration from the pinned Scribe 3.5.4
image, starts a uniquely named PostgreSQL container with no network or host
ports, and runs four rollback-only cases. Its disposable containers and anonymous
volumes are removed on completion.

| Migration | Empty target schema | An unrelated schema already has `__event_type` |
| --- | --- | --- |
| Original packaged SQL | Pass | Fails: missing `__payload_type`, SQLSTATE 42704 |
| In-memory schema-scoped guard | Pass | Pass |

The collision case creates only an unrelated enum, not another PQS writer. The
migration's database-wide `pg_type` lookup finds that name and skips creating its
own schema-local type group. Later DDL cannot resolve `__payload_type` in the
new target schema. Thus the failure is independent of our configuration and
application processing.

The tested image digest is
`sha256:072faffbee3640448b282fc973b04ec99f5ba2bf92d2b43d01a00c89626c3a67`.
Its V001 SQL is byte-for-byte identical to upstream commit
`3be45e0431be572e5b0426cd4b2670f1873a17e7`, with SHA-256
`eb48537d88aca9b8c885becf13ceee769512a1de3b05b569c106487754f44d66`.
The four cases also passed when the script read that checkout's file directly:

```bash
python3 repros/pqs-schema-type-collision/run.py --migration-file \
  /path/to/participant-query-store/modules/postgres/document/resources/db/migration/V001__Create_initial_schema.sql
```

The experiment adds a `typnamespace = current_schema()::regnamespace` condition
in memory. It does not modify upstream files, installed migrations or checksums.
A production correction needs a maintainer-approved, checksum-safe approach:
upstream rules treat merged versioned migrations as immutable. This is a
confirmed SQL-level defect and tested correction candidate, not a built/tested
patch to the current PQS executable. The full upstream Scala suite was not run.

Issue/PR searches for schema, namespace, pg_type and __event_type found no exact
match at this check. [Issue 203](https://github.com/digital-asset/participant-query-store/issues/203)
is related to non-public schemas but reports a missing procedure rather than
this deterministic type collision; do not assume they share a cause.

Sources: [exact migration](https://github.com/digital-asset/participant-query-store/blob/3be45e0431be572e5b0426cd4b2670f1873a17e7/modules/postgres/document/resources/db/migration/V001__Create_initial_schema.sql),
[migration rules](https://github.com/digital-asset/participant-query-store/blob/3be45e0431be572e5b0426cd4b2670f1873a17e7/.claude/rules/sql-and-migrations.md).
Published on user request:

- [Upstream issue #240](https://github.com/digital-asset/participant-query-store/issues/240).
- [Fix PR #241](https://github.com/digital-asset/participant-query-store/pull/241), draft.
- [CantonGrid tracking issue #1](https://github.com/kunal171/cantongrid/issues/1); no CantonGrid PR.

The PR bootstraps the original type definitions in `beforeMigrate` only for the
foreign-type collision, leaving every versioned migration unchanged. PostgreSQL
14.25 checks passed for all current versioned/repeatable SQL, matching baseline
schema dumps and repeated callbacks on an initialized store. A functional
regression test was added and formatted with scalafmt 3.8.3; the full Mill suite
was not run locally because Nix/Mill are unavailable. The [original draft](issue-draft.md)
is retained as preparation history; the published issue has the shorter final text.
