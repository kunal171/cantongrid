# CLI reference

Build with `cargo +1.100.0 build -p cantongrid-cli --locked` and run
`target/debug/cantongrid --help`. [Quickstart](guide/quickstart.md) covers PATH setup.

| Command | Purpose |
| --- | --- |
| `upgrade plan <old-runtime> <new-runtime> <dar> <models> <namespace>` | Read-only reviewed manifest for installed targets |
| `upgrade start\|step\|status\|activate\|cancel <manifest> <old-runtime> <new-runtime> <dar> <models>` | Durable paged handoff; see [upgrades](guide/upgrades.md) |
| `upgrade apply <manifest> <old-runtime> <new-runtime> <dar> <models>` | Bounded atomic alternative |
| `metadata inspect <artifact.dar>` | Offline LF 2.1 signatures as bounded JSON; source-only alpha.2 |
| `init <new-directory>` | Scaffold a synthetic Visibility fixture; refuses an existing directory |
| `validate <project.json>` | Validate fixture configuration without reading events or connecting |
| `fixture run <project.json> [--party A]...` | In-memory fixture replay with simulated grants |
| `storage init --database-env CANTONGRID_DESTINATION_URL` | Explicit owned destination migrations |
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
| `models register <models.json> --namespace <project> --database-env <variable>` | Atomically register immutable definitions; no activation |
| `models inspect <project> <model-name> <revision> --database-env <variable>` | Read registration/fingerprint metadata |
| `models bind <runtime.json> <project> <model-name> <revision> [--replace <previous-revision>]` | Verify source rights/readiness, atomically backfill and activate an installed revision |
| `models rebuild start <runtime.json> <job> <project> <model-name> <revision> [--replace <previous-revision>]` | Reserve a private inactive target and persist a rebuild job |
| `models rebuild step <runtime.json> <job> [--limit <1..64>]` | Commit one bounded build/reconciliation/cleanup page; default 32 |
| `models rebuild status <runtime.json> <job>` | Inspect operator progress under current source rights |
| `models rebuild activate <runtime.json> <job>` | Switch a ready, reconciled target into the active binding |
| `models rebuild cancel <runtime.json> <job>` | Request bounded cleanup before activation |
| `models install <project> <model-name> <revision> --database-env <variable>` | Create/verify a private empty table and link its revision; no activation |

Arguments and ordering are strict. Credentials are read from explicitly named
`CANTONGRID_*` variables. Fixture documents are bounded to 8 MiB; runtime/model
configuration to 64 KiB. Unknown/duplicate fields and unsupported versions fail
with redacted errors. Runtime, fixture and model JSON are different schemas.

Workflows: [fixtures/PostgreSQL](guide/quickstart.md), [LocalNet](guide/localnet-source.md),
[API](guide/api.md), [models](guide/models.md). `dev`, `doctor`, live `status`,
stream commands are not implemented. Generated model reads use `/models/graphql`;
see the [PoC](guide/poc.md) for a released example.
