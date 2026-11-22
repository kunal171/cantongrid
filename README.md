# CantonGrid

A Rust indexer for authorized Daml contracts on Canton. It maintains active
contracts and lifecycle history in PostgreSQL and serves caller-scoped REST/GraphQL.
Outcomia is the first application example; the engine is reusable.

[Runnable PoC](docs/guide/poc.md) · [Developer guide](docs/README.md) · [Architecture](docs/architecture.md) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

[Latest release: 0.3.0](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0)

## Current status

The source checkout prepares **0.4.0-alpha.4** with opt-in, one-hop
[direct lineage](docs/guide/lineage.md), migration 010 and generic/Outcomia LocalNet
acceptance. This work has not been published.

The **0.4.0-alpha.3** prerelease adds opt-in internal exercise
receipts and atomic transaction ingestion, verified on the pinned LocalNet.
See the [receipt profile](docs/design/exercise-ingestion.md) for migration 009,
rollout and unsupported cases.

- LocalNet PQS snapshot/update ingestion, atomic checkpoints and authenticated
  generic APIs are verified for the [tested profile](docs/verification.md).
- Model validation, exact calculations, registry and private table installation are available.
- Bounded model backfill/promotion and atomic live projections have owned-Postgres tests.
- Generated REST/GraphQL model queries support exact filters, sorting, discovery
  and query-bound cursors, verified on LocalNet with the standalone PoC and Outcomia.
- Staged rebuilds passed 261-contract LocalNet acceptance with source revocation,
  concurrent events, promotion and rollback. PQS/destination fixtures use PostgreSQL 17.12.
- Durable streams remain planned.
- Remote/devnet/production deployment is unverified.

```bash
cargo install cantongrid-cli --version 0.3.0 --locked
cantongrid init ./my-demo
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

See the [quickstart](docs/guide/quickstart.md) for setup and examples, and the
[next steps](docs/README.md#next) for remaining work. Runtime code is Rust; Python scripts
are local development tools. Licensed under [Apache-2.0](LICENSE).

The alpha.4 source checkout also includes [declared relationships](docs/guide/relationships.md):
versioned application links over exact model keys, with independently visible
endpoints and generated REST/GraphQL client contracts. They are separate from
source-proven lineage. See the [example](examples/relationships/README.md).
