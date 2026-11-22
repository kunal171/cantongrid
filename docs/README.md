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
| [Package upgrades](guide/upgrades.md) | Reviewed additive handoffs, staged publication and rollback (unreleased) |
| [DAR metadata](guide/metadata.md) | Offline package identities and qualified contract signatures (unreleased) |
| [Developer inspector](guide/inspector.md) | Authorized committed records, projection findings and local progress |
| [Typed inner joins](guide/joins.md) | Selected typed values with complete input visibility |
| [Declared relationships](guide/relationships.md) | Versioned application links with independent endpoint visibility |
| [Aggregates](guide/aggregates.md) | Grouped counts/sums with full-source authorization |
| [Outcomia](guide/outcomia.md) | Prediction-market mappings and application acceptance |
| [Operations](guide/operations.md) | Recovery, troubleshooting and disk usage |

Reference: [architecture](architecture.md), [security](security.md),
[commands](cli.md), [fixture format](fixtures.md), [verification](verification.md),
[releases](releases.md), [contributing](https://github.com/kunal171/cantongrid/blob/main/CONTRIBUTING.md).

## Current scope

The latest stable release is **0.3.0**; the latest prerelease is **0.4.0-alpha.4**.
The alpha series adds aggregates, metadata/sync, exercises, direct lineage,
declared relationships and typed inner joins. Stable 0.3.0 includes LocalNet ingestion, model projections,
generated TypeScript/GraphQL, filters, sorting, discovery and staged rebuilds.
PostgreSQL 17.12 compatibility and large real-ledger rebuild acceptance pass.
See [verification](verification.md) for the exact environments and checks.

The [aggregate slice](guide/aggregates.md) adds count/sum queries over
maintained projections, generated APIs and complete-source access checks.

Phase 4 model integration remains in progress. History starts at an active
snapshot and continues forward. Published 0.3.0 does not include aggregates or joins. Interfaces/reassignments,
Scan, hosted multi-tenancy and arbitrary processing plugins are not implemented.
Remote/TLS, devnet and production deployment are unverified.

## Next

Release 0.4.0-alpha.4 includes [direct lineage](guide/lineage.md).
The published alpha.3 provides internal exercise receipts. Both retain
0.4.0-alpha.2's offline DAR inspection (#23),
compound sorting (#21), reviewed package upgrades (#25), verified remote HTTPS
transport (#22), and command-driven schema synchronization (#24).

1. [M0 exercise-source feasibility](design/exercise-source.md) is verified on the
   pinned LocalNet: four party scopes match Ledger Effects, with transient and
   nested controls. General immediate-parent proof remains unsupported.
2. [M1 internal exercise receipts](design/exercise-ingestion.md) is implemented
   in this checkout with explicit tree-mode opt-in and destination migration 009.
   [M2 direct lineage](design/lineage.md) is verified on the pinned LocalNet in this
   checkout. [M3 declared relationships](design/relationships.md) is implemented in this checkout;
   [M4 typed inner joins](design/joins.md) is implemented with owned PostgreSQL/mock-source checks.
   [M5 developer inspector](design/inspector.md) is implemented in the alpha.5
   source checkout. Historical queries and durable streams follow their own gates.
3. Extend existing metadata/sync with impact analysis rather than duplicating it.
   Outcomia factory/clubs contract work remains a separate repository track.

## Publishing

[Public guide](https://shady-3.gitbook.io/cantongrid/). The same Markdown is used
for GitBook: `.gitbook.yaml` selects this page and `SUMMARY.md`. Publishing currently
uses reviewed GitBook change requests; automatic Git Sync is not configured.
Editing these files alone does not update the published site.
