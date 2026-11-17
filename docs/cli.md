# CLI

Status: fixture workflows and the [LocalNet index/serve profile](live-runtime.md) are implemented. Orchestration, generated models and streams remain planned.

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

## Implemented durable fixture workflow

Start the separately owned local destination (no Canton/ledger access required):

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo run -p cantongrid-cli -- storage init --database-env CANTONGRID_DESTINATION_URL
cargo run -p cantongrid-cli -- fixture index examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cargo run -p cantongrid-cli -- fixture status examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cargo run -p cantongrid-cli -- fixture query examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL --party A
```

Repeat `fixture index` to verify zero applied batches and unchanged state/outbox.
Use the Outcomia example project with the same commands. `query` rebuilds its
reference engine from the destination journal; the original fixture file is not
required for querying. Repeat `--party` to simulate A + B; no parties yields zero
visible records. These synthetic grants are not an authentication API. `status`
and ingestion summaries are explicitly operator counts, not filtered user counts.

Only `storage init` runs destination DDL. Indexing refuses an uninitialized or
incompatible destination. The source connection can never be used for implicit
migrations. Each delivery is atomic; a later invalid batch leaves earlier committed
batches intact and reports failure. Recovery, size limits, ownership checks and
commit-acknowledgement uncertainty are described in [storage](storage.md).

Credentials are read only from an explicitly named `CANTONGRID_*` environment
variable, not inline CLI arguments or project JSON. The credentials above are
public disposable fixture defaults. This transport is loopback-only; production
connections, TLS, retention and application authentication are not implemented.

## Implemented PQS inspection

Against the existing, separately provisioned schema-041 compatibility fixture:

```bash
export CANTONGRID_PQS_URL='host=127.0.0.1 port=5441 user=cantongrid_pqs_reader password=local-pqs-reader dbname=pqs'
cargo run -p cantongrid-cli -- source inspect-pqs --database-env CANTONGRID_PQS_URL
```

This is a read-only operator probe of SQL metadata, temporal bounds and row counts.
It copies no payloads into the destination and always reports private lifecycle
ingestion disabled. SQL availability does not establish source-stream health or
current ledger grants. See the [archive evidence restriction](storage.md).

## Retained source-window verification

```bash
cargo run -p cantongrid-cli -- source verify-window --database-env CANTONGRID_PQS_URL --after 80
```

The source variable uses the existing SELECT-only schema-041 fixture from the
inspection example. The command compares active-state identity sets against
creates/archives in a retained interval inside one read-only transaction. It
returns counts only, never private payloads or party identifiers. A consistent
historical interval is not a concurrent publication/pagination proof; the result
explicitly reports `concurrent_handoff_verified=false`. Invalid, unavailable or
pruned bounds fail instead of advancing a checkpoint. No source or destination
writes occur. See the [source contract](phase3-source-contract.md).

## Verify current source grants

```bash
cantongrid source verify-grants .local/source-grants.json
```

The [grant-check configuration and constraints](source-authorization.md) are
separate from fixture projects. The command uses a configured credential environment
variable, makes read-only authenticated Ledger requests, and prints no party IDs
or token values. Failure clears the renewal state; success is a bounded current
scope check, not a private ingestion or complete-history claim.


## Live contracts

```bash
cantongrid index .local/runtime.json --once
cantongrid index .local/runtime.json
cantongrid serve .local/runtime.json
```

Use the separate [runtime configuration](live-runtime.md), exact read-only source
access and initialized owned destination. `--once` processes one bounded page;
continuous indexing retries without dropping filters or uncommitted checkpoints.
The API uses independently authenticated caller tokens, not fixture party flags.


## Declarative model tools (Phase 4 in progress)

```bash
cantongrid models validate examples/models/outcomia.json
cantongrid models plan examples/models/visibility.json
cantongrid models preview examples/models/visibility.json records /path/to/payload.json
```

See [the model contract](phase4-models.md). These bounded offline tools validate,
evaluate or generate a reviewable SQL plan; they do not install schemas or change
the live worker/API. Preview uses an explicitly supplied local payload, not source
or caller authentication. Model materialization and generated queries are next.
