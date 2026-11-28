# CantonGrid

CantonGrid is a Rust indexer for authorized Daml contracts on Canton.
- **Storage:** it keeps active contracts and lifecycle history in PostgreSQL.
- **APIs:** it serves REST and GraphQL scoped to each caller.
- **Typed models:** it generates typed models, TypeScript and GraphQL from
  declarative mappings.

Outcomia is the first application example; the engine is generic.

**Versions.**
- **Latest stable:** **0.4.0**.
- **Latest prerelease:** **0.5.0-alpha.2**, which adds reconciliation.

**Supported profile.**
- **Software:** authorized node-local Canton 3.5.7, Scribe 3.5.4 (schema041) and
  PostgreSQL 17.12.
- **History:** from the snapshot forward.
- **Remote:** remote networks (TestNet and MainNet) are unverified.

| Guide | Covers |
| --- | --- |
| [Quickstart](guide/quickstart.md) | Install, fixtures, PostgreSQL |
| [LocalNet](guide/localnet.md) | Local stack, indexing your contracts, the PoC |
| [API](guide/api.md) | Keys, grants, contracts, history, model queries |
| [Models](guide/models.md) | Definitions, install and bind, staged rebuilds, aggregates |
| [Composition](guide/composition.md) | Lineage, declared relationships, joins, historical reads |
| [Delivery](guide/delivery.md) | Durable subscriptions and atomic bootstrap |
| [Reconciliation](guide/reconciliation.md) | Exact per-contract invariant checks (0.5 alpha) |
| [Upgrades](guide/upgrades.md) | DAR metadata, impact analysis, package handoff |
| [Operations](guide/operations.md) | Readiness, remote sources, backup, inspector, troubleshooting |
| [Outcomia](guide/outcomia.md) | The prediction-market example |

**Reference:**
- **System:** [architecture](architecture.md) and [security](security.md).
- **Usage:** [commands and fixtures](cli.md).
- **Evidence and history:** [verification](verification.md) and
  [releases](releases.md).
- **Design contracts:** [exercises](design/exercises.md),
  [composition](design/composition.md), [delivery](design/delivery.md) and
  [reconciliation](design/reconciliation.md).

**Not implemented:**
- **History:** historical model bindings and model CDC.
- **Ledger features:** interfaces and reassignments.
- **Platform:** public Scan, hosted multi-tenancy and processing plugins.

## Publishing

- **Site:** these files are the [public guide](https://shady-3.gitbook.io/cantongrid/).
- **Sync:** GitBook Git Sync publishes `docs/` from `main`, using `SUMMARY.md` for
  navigation.
- **Changes:** go through pull requests.
- **Contributing:**
  [CONTRIBUTING.md](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).
