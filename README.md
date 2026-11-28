# CantonGrid

A Rust indexer for authorized Daml contracts on Canton.
- **Storage:** it keeps active contracts and lifecycle history in PostgreSQL.
- **APIs:** it serves REST and GraphQL scoped to each caller.
- **Typed models:** it generates typed models with TypeScript and GraphQL clients.

Outcomia is the first application example; the engine is generic.

[Guide](docs/README.md) · [Quickstart](docs/guide/quickstart.md) ·
[PoC](docs/guide/localnet.md#the-poc) · [Architecture](docs/architecture.md) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

[Latest stable: 0.4.0](https://github.com/kunal171/cantongrid/releases/tag/v0.4.0)
· Prerelease: 0.5.0-alpha.2 (reconciliation)

```bash
cargo install cantongrid-cli --version 0.4.0 --locked
cantongrid init ./my-demo
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

## What it does

- **Ingestion:** reads from the snapshot forward, with atomic checkpoints and
  separate source and caller grants.
- **Models:** exact projections, generated TypeScript and GraphQL, filters,
  sorting, staged rebuilds and aggregates.
- **Composition:** direct lineage, declared relationships, typed joins and a
  developer inspector.
- **Delivery:** at-least-once durable delivery with an atomic bootstrap.
- **Operations:** readiness checks, preflight, backup and restore checks, and
  reviewed package upgrades.

**Supported profile.**
- **Software:** node-local Canton 3.5.7, Scribe 3.5.4 (schema041) and PostgreSQL
  17.12.
- **Experimental:** temporal reads.
- **Not supported:** historical model bindings, model CDC, interfaces and
  reassignments, multi-tenancy and public Scan.
- **Unverified:** remote networks (TestNet and MainNet) and production use.

Evidence for each feature is in [verification](docs/verification.md), and upgrade
steps are in [releases](docs/releases.md#upgrading).

## Notes

- **Language:** the runtime is Rust. The Python scripts are local development
  tools.
- **License:** [Apache-2.0](LICENSE).
