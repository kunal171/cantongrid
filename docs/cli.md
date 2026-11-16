# Planned CLI

Status: command design only. No command in this document is implemented or
installable yet. Flags, file names and configuration syntax remain provisional.

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
