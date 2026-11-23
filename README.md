# CantonGrid

A Rust indexer for authorized Daml contracts on Canton. It maintains active
contracts and lifecycle history in PostgreSQL and serves caller-scoped REST/GraphQL.
Outcomia is the first application example; the engine is reusable.

[Runnable PoC](docs/guide/poc.md) · [Developer guide](docs/README.md) ·
[Architecture](docs/architecture.md) · [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

[Latest stable release: 0.4.0](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0)

## Current status

**0.4.0** promotes the tested alpha.7 implementation for the authorized node-local
Canton 3.5.7 / Scribe 3.5.4 schema041 / PostgreSQL 17.12 profile. See the
[release scope and upgrade steps](docs/releases.md#040-supported-scope-and-acceptance).

- Snapshot-forward ingestion, atomic checkpoints and current source/caller grants.
- Exact model projections, generated TypeScript/GraphQL, filters, sorting and staged rebuilds.
- Declared relationships, typed joins, aggregates, source evidence and developer inspection,
  with per-feature verification levels recorded in the [guide](docs/verification.md).
- Durable lifecycle delivery and [atomic canonical bootstrap](docs/guide/bootstrap.md),
  including replay, monotonic ACKs, private cursors and explicit retention gaps.
- Readiness/preflight, private backup/restore checks and local benchmark evidence.

Canonical temporal reconstruction remains a bounded experimental prototype.
Historical model bindings, model CDC, interfaces/reassignments, hosted multi-tenancy
and public Scan are outside this release's scope. Remote/TestNet and production
acceptance remain unverified; the stable version does not imply full Phase 4 completion.

```bash
cargo install cantongrid-cli --version 0.4.0 --locked
cantongrid init ./my-demo
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

The [quickstart](docs/guide/quickstart.md) covers setup and examples. Back up before
explicitly migrating an older owned destination through 012; source PQS is never
migrated. Alpha.7 installations need no new migration or reindex for 0.4.0.
Runtime code is Rust; Python scripts are local development tools.
Licensed under [Apache-2.0](LICENSE).
