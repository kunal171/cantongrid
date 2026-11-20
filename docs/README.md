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
| [Aggregates](guide/aggregates.md) | Grouped counts/sums with full-source authorization |
| [Outcomia](guide/outcomia.md) | Prediction-market mappings and application acceptance |
| [Operations](guide/operations.md) | Recovery, troubleshooting and disk usage |

Reference: [architecture](architecture.md), [security](security.md),
[commands](cli.md), [fixture format](fixtures.md), [verification](verification.md),
[releases](releases.md), [contributing](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).

## Current scope

The latest stable release is **0.3.0**; the source prerelease **0.4.0-alpha.1**
adds grouped aggregates. Stable 0.3.0 It includes LocalNet ingestion, model projections,
generated TypeScript/GraphQL, filters, sorting, discovery and staged rebuilds.
PostgreSQL 17.12 compatibility and large real-ledger rebuild acceptance pass.
See [verification](verification.md) for the exact environments and checks.

The [aggregate slice](guide/aggregates.md) adds count/sum queries over
maintained projections, generated APIs and complete-source access checks.

Phase 4 model integration remains in progress. History starts at an active
snapshot and continues forward. Published 0.3.0 does not include aggregates. Joins, interfaces/reassignments,
Scan, hosted multi-tenancy and arbitrary processing plugins are not implemented.
Remote/TLS, devnet and production deployment are unverified.

## Next

1. Extend Outcomia projections to funded assets/trading after separate authorization.
2. Define joins and broader aggregate operations with retraction and visibility
   for every input before implementation. Phase 5 then adds authenticated resumable streams, retention-gap
   handling and reauthorization; existing outbox rows are not a public stream.

## Publishing

[Public guide](https://shady-3.gitbook.io/cantongrid/). The same Markdown is used
for GitBook: `.gitbook.yaml` selects this page and `SUMMARY.md`. Publishing currently
uses reviewed GitBook change requests; automatic Git Sync is not configured.
Editing these files alone does not update the published site.
