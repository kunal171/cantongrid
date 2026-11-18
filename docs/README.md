# CantonGrid documentation

CantonGrid indexes authorized Daml contracts into PostgreSQL and exposes
caller-scoped REST/GraphQL. Outcomia is an example, not an engine dependency.

| Start here | Covers |
| --- | --- |
| [LocalNet PoC](guide/poc.md) | Deploy a Daml contract, generate types, transact and verify indexing |
| [Quickstart](guide/quickstart.md) | Build, fixtures and PostgreSQL |
| [LocalNet](guide/localnet-source.md) | Source setup, runtime configuration and ingestion |
| [API](guide/api.md) | Caller authentication, REST, GraphQL and pagination |
| [Models](guide/models.md) | Typed definitions, exact calculations, source bindings and live projections |
| [Outcomia](guide/outcomia.md) | Prediction-market fixtures and model example |
| [Operations](guide/operations.md) | Recovery, troubleshooting and disk usage |

Reference: [architecture](architecture.md), [security](security.md),
[CLI](cli.md), [fixture format](fixtures.md), [verification](verification.md),
[roadmap](roadmap.md), [releases](releases.md), [contributing](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).

The LocalNet contracts runtime is verified. Version 0.3.0-alpha.1 adds model
validation, exact calculations, registry, installation, bounded source-bound
backfill/promotion and atomic live projections. Projection controls use owned
PostgreSQL and synthetic source authority; actual LocalNet model acceptance and
generated typed APIs remain pending in that release. The [PoC branch](guide/poc.md)
adds generated model GraphQL/client contracts and actual LocalNet acceptance.
Streams are later work.
Remote/devnet/production deployment has not been verified.

## Publishing

The same Markdown is prepared for GitBook Git Sync. Repository-root
`.gitbook.yaml` selects `docs/README.md` and `docs/SUMMARY.md`. Map the GitBook
space to the repository root and sync the maintained branch. Hosting is a separate
step; a configuration file alone does not publish a site. The current
[public guide](https://shady-3.gitbook.io/cantongrid/) is updated through reviewed
GitBook change requests; automatic Git Sync is not configured.
