# CantonGrid

**Open-source indexing infrastructure for Canton. Automatically turn authorized
Daml contracts into queryable PostgreSQL databases, GraphQL APIs, and real-time
data streams.**

Status: the [LocalNet contracts runtime](docs/live-runtime.md) ingests authorized
PQS snapshots and forward lifecycle events into owned PostgreSQL and serves
caller-scoped REST/GraphQL. Actual Outcomia and unrelated Visibility examples
passed through the same Rust engine. [Phase 3 evidence](docs/phase3-verification.md)
records the tested limits: snapshot-forward partial history, raw exact JSON,
loopback-only transport and no production claim. Generated business models and
real-time streams remain Phase 4/5.

The indexer runtime is **Rust**. Python scripts are development helpers for
LocalNet and compatibility checks, not the ingestion or API implementation.

Developers describe **what data to process** and **how to process it**.
CantonGrid handles the supported ingestion, lifecycle tracking, projections,
schema management, APIs, change delivery, and recovery infrastructure.
Deployment still requires an operator to supply authorized source access,
storage, credentials and access policy. The engine cannot manufacture permissions
or infer application accounting rules.

Outcomia is the first live application example to build. It must be expressed through
configuration and verified package metadata, without Outcomia-specific engine
code. A second unrelated Daml example must work without changing Rust code.

The first release is contracts-only, developed against LocalNet while requested
devnet infrastructure is pending. One organization may issue tokens restricted
to permitted party subsets. Other Canton data sources remain future extensions.

Planned primary commands:

```bash
cantongrid init    # Scaffold source/access settings and processing definitions
cantongrid index   # Bootstrap and continuously maintain indexed data
cantongrid serve   # Serve generated queries and resumable change streams
```

Read the [documentation index](docs/README.md),
[product contract](docs/product.md), and
[accepted decisions](docs/decisions.md). Start implementation planning with the
[architecture](docs/architecture.md) and [phased roadmap](docs/roadmap.md).

This is a separate repository. Neither Outcomia repository is a platform
dependency or a write target. The working name is provisional; the project is licensed under [Apache-2.0](LICENSE). Third-party dependencies
retain their own licenses and are checked by the dependency policy.

Development follows [versioned milestones](docs/releases.md). See
[CONTRIBUTING.md](CONTRIBUTING.md) for setup, tests and contribution boundaries,
and [SECURITY.md](SECURITY.md) for private vulnerability reporting.
