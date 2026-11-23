# CantonGrid documentation

CantonGrid indexes authorized Daml contracts into PostgreSQL and serves
caller-scoped REST/GraphQL. Outcomia is the first application example.

| Guide | Covers |
| --- | --- |
| [LocalNet PoC](guide/poc.md) | Build and deploy a Daml contract, generate types, transact and verify indexing |
| [Quickstart](guide/quickstart.md) | Build, synthetic fixtures and PostgreSQL |
| [LocalNet ingestion](guide/localnet-source.md) | Source setup, runtime configuration and worker |
| [API](guide/api.md) | Caller authentication, queries, filtering and pagination |
| [Models](guide/models.md) | Definitions, exact calculations, installation and rebuilds |
| [Package upgrades](guide/upgrades.md) | Reviewed additive handoffs, staged publication and rollback |
| [DAR metadata](guide/metadata.md) | Offline package identities and qualified contract signatures |
| [Upgrade impact analysis](guide/upgrade-impact.md) | Offline dependency, generated API and rebuild review |
| [Developer inspector](guide/inspector.md) | Authorized committed records, projection findings and local progress |
| [Typed inner joins](guide/joins.md) | Selected typed values with complete input visibility |
| [Declared relationships](guide/relationships.md) | Versioned application links with independent endpoint visibility |
| [Aggregates](guide/aggregates.md) | Grouped counts/sums with full-source authorization |
| [Outcomia](guide/outcomia.md) | Prediction-market mappings and application acceptance |
| [Temporal state](guide/temporal.md) | Bounded canonical as-of reads |
| [Durable delivery](guide/delivery.md) | Private subscriptions, replay, ACK and explicit gaps |
| [Reconciliation (0.5 source)](guide/reconciliation.md) | Offline policies, schema preflight and bounded indexed per-contract checks |
| [Bootstrap](guide/bootstrap.md) | Opaque fixed baseline paired with a durable feed |
| [Remote setup](guide/remote-source.md) | HTTPS, restricted PQS tunnel and preflight checks |
| [Recovery](guide/recovery.md) | Private isolated backup/restore validation |
| [Operations](guide/operations.md) | Readiness, pressure, troubleshooting and disk usage |

Reference: [architecture](architecture.md), [security](security.md),
[commands](cli.md), [fixture format](fixtures.md), [verification](verification.md),
[releases](releases.md), [contributing](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).

## Current scope

Latest stable: **0.4.0**. Latest prerelease: **0.5.0-alpha.1**.
0.4.0 promotes alpha.7's implementation without changing runtime code or migrations.
Its supported live profile is authorized node-local Canton 3.5.7, Scribe 3.5.4
schema041 and PostgreSQL 17.12, with snapshot-forward partial history.
See [release scope](releases.md#040-supported-scope-and-acceptance) and the
per-feature [verification record](verification.md).

The release includes exact models/generated queries, scoped composition and
inspection, durable lifecycle delivery and [atomic canonical bootstrap](guide/bootstrap.md).
Some composition/concurrency checks use owned PostgreSQL and mock source authority;
retained LocalNet evidence is documented separately. Canonical temporal queries
remain a bounded experimental prototype with explicit unavailable coverage.

Historical model bindings, model CDC, interfaces/reassignments, public Scan,
hosted multi-tenancy and arbitrary processing plugins are not implemented.
Remote/TestNet and production acceptance remain unverified. Local HTTPS certificate,
backup and benchmark controls do not establish live-network or production readiness.

Prerelease **0.5.0-alpha.1** adds
[reconciliation policies](guide/reconciliation.md), schema preflight and storage-only
indexed per-contract checks. Fixture reports remain unverified; indexed checks
cover a bounded retained prefix. Public APIs and cross-contract totals remain later work.

## Milestones

M0–M6 add source feasibility, exercise receipts, direct lineage, declared links,
typed joins, developer inspection and upgrade impact. M7/M8 add bounded canonical
temporal reads and durable delivery; alpha.7 adds atomic bootstrap. M9 local
hardening/tooling is included, while its remote/production gates remain pending.
Phase 4 and Outcomia factory/clubs contracts remain separate continuing work.

## Publishing

[Public guide](https://shady-3.gitbook.io/cantongrid/). The same Markdown is used
for GitBook: `.gitbook.yaml` selects this page and `SUMMARY.md`. Publishing currently
uses reviewed GitBook change requests; automatic Git Sync is not configured.
Editing these files alone does not update the published site.
