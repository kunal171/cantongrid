Initial migration skips schema-local types when another schema has __event_type

Scribe 3.5.4/schema 041 fails to initialize a target schema if an unrelated schema
in the same database contains an enum named `__event_type`. V001 queries pg_type
by typname without restricting typnamespace, skips the target's type creation,
then fails with SQLSTATE 42704: type "__payload_type" does not exist.

The accompanying standalone run.py reproduces this using PostgreSQL 14.25 and
the unmodified migration extracted from image digest
sha256:072faffbee3640448b282fc973b04ec99f5ba2bf92d2b43d01a00c89626c3a67.
It needs no ledger, contracts or application code. The clean-schema control
passes; adding an unrelated schema containing only that enum triggers the failure.

```bash
python3 run.py
```

The same V001 file is present at upstream commit
3be45e0431be572e5b0426cd4b2670f1873a17e7, and the reproduction also passes its
expected-failure/control checks against that file. Adding a current-schema
namespace condition in memory makes both controls pass. This is not an in-place
migration patch; a correction must preserve validation of deployed Flyway histories.
The current executable and full Scala suite have not been built/tested here.

Issue #203 concerns non-public schemas but has a different observed error; this
report does not assume the same cause.
