# CantonGrid

**Open-source indexing infrastructure for Canton. Automatically turn authorized
Daml contracts into queryable PostgreSQL databases, GraphQL APIs, and real-time
data streams.**

Status: Rust configuration and fixture lifecycle processing are implemented. The
[CLI](docs/cli.md) can persist synthetic input to an owned local PostgreSQL store,
recover state and query simulated party scopes. [Durable ingestion evidence and
limits](docs/storage.md) distinguish this bounded reference implementation from
live source ingestion. A read-only PQS inspector reports source capabilities and
open gates. A separate [source authorization probe](docs/source-authorization.md)
checks current LocalNet read-only grants without copying contracts.
Live contract copying, authenticated REST/GraphQL and streams remain
planned. [LocalNet development helpers](docs/localnet.md) support compatibility research.

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
