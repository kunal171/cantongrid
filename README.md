# CantonGrid

**Open-source indexing infrastructure for Canton. Automatically turn authorized
Daml contracts into queryable PostgreSQL databases, GraphQL APIs, and real-time
data streams.**

Status: documentation foundation complete; LocalNet compatibility work is in
progress. Development helpers are available in the [LocalNet guide](docs/localnet.md).
There is no CantonGrid executable, live connector, database migration or API
server yet. Product commands and interfaces below remain planned.

Developers describe **what data to process** and **how to process it**.
CantonGrid handles the supported ingestion, lifecycle tracking, projections,
schema management, APIs, change delivery, and recovery infrastructure.
Deployment still requires an operator to supply authorized source access,
storage, credentials and access policy. The engine cannot manufacture permissions
or infer application accounting rules.

Outcomia is the first working example to build. It must be expressed through
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
dependency or a write target. The working name is provisional; the license target
is Apache-2.0, pending license and dependency review before publication.
