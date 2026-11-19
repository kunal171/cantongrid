# CantonGrid

A Rust indexer for authorized Daml contracts on Canton. It maintains active
contracts and lifecycle history in PostgreSQL and serves caller-scoped REST/GraphQL.
Outcomia is the first application example; the engine is reusable.

[Runnable PoC](docs/guide/poc.md) · [Developer guide](docs/README.md) · [Architecture](docs/architecture.md) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

[Latest source prerelease: 0.3.0-alpha.4](https://github.com/kunal171/cantongrid/releases/tag/v0.3.0-alpha.4)

## Current status

- LocalNet PQS snapshot/update ingestion, atomic checkpoints and authenticated
  generic APIs are verified for the [tested profile](docs/verification.md).
- Model validation, exact calculations, registry and private table installation are available.
- Bounded model backfill/promotion and atomic live projections have owned-Postgres tests.
- Generated REST/GraphQL model queries support exact filters, sorting, discovery
  and query-bound cursors, verified on LocalNet with the standalone PoC and Outcomia.
- Staged large backfills and durable streams remain planned.
- Remote/devnet/production deployment is unverified.

```bash
cargo +1.100.0 build -p cantongrid-cli --locked
./target/debug/cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A
```

See the [quickstart](docs/guide/quickstart.md) for setup and examples, and the
[roadmap](docs/roadmap.md) for remaining work. Runtime code is Rust; Python scripts
are local development tools. Licensed under [Apache-2.0](LICENSE).
