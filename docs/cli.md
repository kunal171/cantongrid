# CLI reference

Build with `cargo +1.100.0 build -p cantongrid-cli --locked` and run
`target/debug/cantongrid --help`. [Quickstart](guide/quickstart.md) covers PATH setup.

| Command | Purpose |
| --- | --- |
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
| `models plan <models.json>` | Print DDL without executing it |
| `models register <models.json> --namespace <project> --database-env <variable>` | Atomically register immutable definitions; no activation |
| `models inspect <project> <model-name> <revision> --database-env <variable>` | Read registration/fingerprint metadata |

Arguments and ordering are strict. Credentials are read from explicitly named
`CANTONGRID_*` variables. Fixture documents are bounded to 8 MiB; runtime/model
configuration to 64 KiB. Unknown/duplicate fields and unsupported versions fail
with redacted errors. Runtime, fixture and model JSON are different schemas.

Workflows: [fixtures/PostgreSQL](guide/quickstart.md), [LocalNet](guide/localnet-source.md),
[API](guide/api.md), [models](guide/models.md). `dev`, `doctor`, live `status`,
managed model installation and stream commands are not implemented.
