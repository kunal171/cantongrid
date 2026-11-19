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
| [Outcomia](guide/outcomia.md) | Prediction-market mappings and application acceptance |
| [Operations](guide/operations.md) | Recovery, troubleshooting and disk usage |

Reference: [architecture](architecture.md), [security](security.md),
[commands](cli.md), [fixture format](fixtures.md), [verification](verification.md),
[releases](releases.md), [contributing](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).

## Current scope

The latest GitHub source prerelease is **0.3.0-alpha.4**: LocalNet ingestion,
model projections, generated TypeScript/GraphQL, filters, sorting and discovery.
The current checkout is **0.3.0-alpha.5, unreleased**, adding staged rebuilds and
security fixes with owned-PostgreSQL and mock-source HTTP coverage. See
[verification](verification.md) for the environments actually tested.

Phase 4 model integration remains in progress. History starts at an active
snapshot and continues forward. Joins, aggregates, interfaces/reassignments,
Scan, hosted multi-tenancy and arbitrary processing plugins are not implemented.
Remote/TLS, devnet and production deployment are unverified.

## Next

1. Validate PQS and destination compatibility on a supported PostgreSQL major
   version, then plan migration of the pinned PostgreSQL 14 fixtures.
2. Run a larger staged rebuild on LocalNet, including ingestion between pages,
   restart, revocation and activation.
3. Extend Outcomia projections to funded assets/trading after separate authorization.
4. Define joins/aggregates with retraction and visibility for every input before
   implementation. Phase 5 then adds authenticated resumable streams, retention-gap
   handling and reauthorization; existing outbox rows are not a public stream.

## Publishing

[Public guide](https://shady-3.gitbook.io/cantongrid/). The same Markdown is used
for GitBook: `.gitbook.yaml` selects this page and `SUMMARY.md`. Publishing currently
uses reviewed GitBook change requests; automatic Git Sync is not configured.
Editing these files alone does not update the published site.
