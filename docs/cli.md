# CLI

Status: a Rust executable implements the fixture workflow below. Live indexing,
serving and orchestration commands in the planned table are not implemented.

## Implemented fixture workflow

Run from the repository with Rust/Cargo 1.100:

```bash
cargo run -p cantongrid-cli -- init /tmp/my-cantongrid-demo
cargo run -p cantongrid-cli -- validate /tmp/my-cantongrid-demo/cantongrid.json
cargo run -p cantongrid-cli -- fixture run /tmp/my-cantongrid-demo/cantongrid.json --party A
cargo run -p cantongrid-cli -- fixture run examples/fixtures/outcomia/cantongrid.json --party A
```

`init` requires a new directory and writes only the synthetic visibility project.
It never overwrites existing files. `validate` checks the strict versioned project
without opening a fixture or connecting to a source. `fixture run` resolves the
fixture relative to the project file, applies it in memory, replays it again, and
prints counts filtered by explicitly simulated caller grants. Repeat `--party`
for A + B; omit it to see zero records. Unsupported/out-of-scope parties fail.
These flags are for synthetic fixtures and do not authenticate real callers.

Documents must be regular UTF-8 files at most 8 MiB. Unknown commands, fields,
adapters/operators, duplicate keys and unsupported versions fail without printing
input values. Only exact package/module/entity selections and full create/archive
history are supported. See [fixture contracts](fixtures.md). `cargo build
--release -p cantongrid-cli` produces `target/release/cantongrid`.

## Planned live commands

| Command | Intended behavior |
| --- | --- |
| `cantongrid init` | Scaffold connection/access settings and selections/processing definitions, with secret placeholders. |
| `cantongrid index` | Validate configuration, bootstrap or resume, and continuously maintain indexed state. |
| `cantongrid index --once` | Catch up to a documented source boundary and exit; report incomplete coverage or failure. |
| `cantongrid serve` | Serve authenticated REST/GraphQL and supported durable streams from PostgreSQL. |
| `cantongrid dev` | Supervise index and serve locally, with explicit local settings and orderly shutdown. |
| `cantongrid doctor` | Check configuration, connectivity, permissions and compatibility without submitting ledger transactions. |
| `cantongrid status` | Report source readiness, coverage, lag, model versions and processing failures with appropriate access controls. |
| `cantongrid schema inspect` | Inspect available verified schemas and unsupported mappings within the authorized scope. |

## Intended workflow

The developer initializes a project, supplies authorized source and PostgreSQL
settings, chooses contract data, and defines any derived models. Validation should
explain missing metadata, incompatible types or unavailable operators before
indexing. An operator controls source credentials, API principals, retention and
breaking-change promotion. None of these commands invents credentials.

`index` owns ingestion and durable progress. `serve` owns query/stream access and
can run separately. `dev` supervises both; it does not silently provision Canton,
create parties, upload packages or submit example transactions.

Scaffolded configuration must separate committed definitions from local secrets.
A default project must not expose unauthenticated private APIs or listen publicly
without an explicit operator setting. Logs and validation errors must redact
credentials and sensitive values.

## Failure and change semantics

Commands must report unsupported source capabilities and incomplete history
explicitly. Restarting must reuse committed checkpoints. Concurrent index workers
for the same scope need ownership/fencing so that two processes cannot corrupt
progress. Shutdown must preserve transaction boundaries.

Nonbreaking managed schema changes follow configured operator policy. Breaking
changes produce a rebuild/promotion plan, not an implicit destructive migration.
The exact command surface for inspection and promotion will be designed when
schema versioning is implemented.
