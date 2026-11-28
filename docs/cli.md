# Commands and fixtures

Build with `cargo +1.100.0 build -p cantongrid-cli --locked` and run
`target/debug/cantongrid --help`. [Quickstart](guide/quickstart.md) covers PATH setup.

| Command | Purpose |
| --- | --- |
| `sync plan <old.dar> <new.dar> <models.json>` | Offline compatibility/dependency/API/rebuild report; M6 additions in alpha.5 |
| `sync summary <old.dar> <new.dar> <models.json>` | Concise offline impact summary |
| `sync evaluate <old.dar> <new.dar> <models.json> <fixtures.json>` | Sanitized old/new projection sample comparison; no LF execution |
| `sync bundle <old.dar> <new.dar> <models.json> <output-directory>` | Offline generation of review bundle (models, SQL, GraphQL, TS); refuses overwriting |
| `upgrade plan <old-runtime> <new-runtime> <dar> <models> <namespace>` | Read-only reviewed manifest for installed targets |
| `upgrade start\|step\|status\|activate\|cancel <manifest> <old-runtime> <new-runtime> <dar> <models>` | Durable paged handoff; see [upgrades](guide/upgrades.md) |
| `upgrade apply <manifest> <old-runtime> <new-runtime> <dar> <models>` | Bounded atomic alternative |
| `metadata inspect <artifact.dar>` | Offline LF 2.1 signatures as bounded JSON |
| `init <new-directory>` | Scaffold a synthetic Visibility fixture; refuses an existing directory |
| `validate <project.json>` | Validate fixture configuration without reading events or connecting |
| `fixture run <project.json> [--party A]...` | In-memory fixture replay with simulated grants |
| `reconcile validate <policy.json>` | Offline exact policy validation; 0.5 source |
| `reconcile evaluate <policy.json> <fixture.json>` | Sanitized fixture comparisons; always unverified |
| `reconcile preflight <policy.json> <catalog.json> <namespace> <model>` | Offline projected-schema compatibility; no active-binding check |
| `storage init --database-env CANTONGRID_DESTINATION_URL` | Explicit owned destination migrations |
| `storage check --database-env CANTONGRID_DESTINATION_URL` | Read-only migration/checksum/ACL validation |
| `ops check-config <runtime> --profile localnet\|testnet\|mainnet` | Offline network/transport policy check |
| `ops preflight <runtime> --profile localnet\|testnet\|mainnet` | Read-only source/package/rights/health and destination checks |
| `ops status <runtime>` | Operator-local progress and delivery pressure |
| `ops prune-deliveries <runtime> --retain <1..100000> --limit <1..256>` | Bounded outbox maintenance; canonical history retained |
| `fixture index <project.json> --database-env <variable>` | Persist fixture deliveries |
| `fixture status <project.json> --database-env <variable>` | Operator counts/progress |
| `fixture query <project.json> --database-env <variable> [--party A]...` | Query stored fixture journal with simulated grants |
| `source inspect-pqs --database-env <variable>` | Read-only schema/role/boundary inspection; not serving authorization |
| `source verify-window --database-env <variable> --after <offset>` | Reconcile a retained historical interval; not concurrent handoff proof |
| `source verify-grants <source-grants.json>` | Check current exact source rights; not caller authentication |
| `index <runtime.json> [--once]` | Live ingestion; `--once` processes at most one bounded page |
| `serve <runtime.json>` | Authenticated generic REST/GraphQL |
| `models validate <models.json>` | Strict offline definition validation |
| `models preview <models.json> <model-name> <payload.json>` | Evaluate an explicitly supplied local payload |
| `models typescript <models.json>` | Emit exact indexed-row interfaces |
| `models graphql <models.json>` | Emit generated model SDL |
| `models plan <models.json>` | Print DDL without executing it |
| `inspect evaluate <models.json> <model-name> <payload.json>` | Local sanitized evaluation finding; no source connection or ledger execution |
| `inspect progress <runtime.json>` | Current source authority/health plus restricted source bounds and owned progress; read-only, operator-local |
| `models register <models.json> --namespace <project> --database-env <variable>` | Atomically register immutable definitions; no activation |
| `models inspect <project> <model-name> <revision> --database-env <variable>` | Read registration/fingerprint metadata |
| `models bind <runtime.json> <project> <model-name> <revision> [--replace <previous-revision>]` | Verify source rights/readiness, atomically backfill and activate an installed revision |
| `models rebuild start <runtime.json> <job> <project> <model-name> <revision> [--replace <previous-revision>]` | Reserve a private inactive target and persist a rebuild job |
| `models rebuild step <runtime.json> <job> [--limit <1..64>]` | Commit one bounded build/reconciliation/cleanup page; default 32 |
| `models rebuild status <runtime.json> <job>` | Inspect operator progress under current source rights |
| `models rebuild activate <runtime.json> <job>` | Switch a ready, reconciled target into the active binding |
| `models rebuild cancel <runtime.json> <job>` | Request bounded cleanup before activation |
| `models install <project> <model-name> <revision> --database-env <variable>` | Create/verify a private empty table and link its revision; no activation |

**Input rules.**
- **Arguments:** argument order is strict.
- **Credentials:** read only from explicitly named `CANTONGRID_*` variables.
- **Size limits:** fixtures are capped at 8 MiB, and runtime and model JSON at
  64 KiB.
- **Validation:** unknown or duplicate fields and unsupported versions fail with
  redacted errors.
- **Schemas:** runtime, fixture and model JSON are different schemas.

Workflows are in [quickstart](guide/quickstart.md), [LocalNet](guide/localnet.md),
[API](guide/api.md) and [models](guide/models.md).

## Fixture format

Fixtures are synthetic, complete create and archive histories. They never contact
Canton or authenticate anyone. See `examples/fixtures`.

**`cantongrid.json`.**
- **Versions:** `version`, `model_version` and `payload_schema_version` are 1, with
  adapter `fixture`.
- **Source fields:** an explicit source, network and generation, unique parties,
  and a relative path to the events file.
- **Templates:** exact 64-character lowercase package IDs and module/entity names,
  selecting both create and archive events.
- **Rejected:** wildcards, live credentials and model declarations.

**Payloads.**
- **Types:** tagged Text, Party, Bool, Int64, Numeric, Optional, Enum, List and
  Record values.
- **Numbers:** they stay canonical strings, with scale 0–37 and 38 digits.
- **Not a Ledger format:** this isn't the Ledger API wire format.

**Replay.**
- **Staging:** each batch is staged before it's published.
- **Duplicates:** an exact duplicate does nothing.
- **Atomic failures:** conflicting identities, contract reuse, unknown or repeated
  archives, and out-of-scope evidence fail atomically.
- **Positions:** ordered by delivery.
- **Visibility:** reads require every visibility clause to intersect the caller and
  the source, and `--party` simulates this.
- **Not supported:** history must start at the origin. Orphan archives, snapshots,
  exercises and reassignments aren't supported.
