# CantonGrid developer guide

CantonGrid indexes authorized Daml contracts into a separately owned PostgreSQL
database and exposes caller-scoped REST and GraphQL APIs. The runtime is Rust.
Developers select templates and define supported processing rules; operators
supply infrastructure, source permissions and API access policy.

This guide covers the current source checkout, targeting `0.3.0-alpha.1`.
The LocalNet runtime was verified in the Phase 3
milestone. Model tools are available offline; live model integration is in progress.

| Capability | Available now | Boundary |
| --- | --- | --- |
| In-memory and PostgreSQL fixtures | Yes | Synthetic grants, no authenticated live source |
| LocalNet PQS ingestion | Yes | Tested schema041, template-only, snapshot-forward partial history |
| REST and GraphQL | Yes | Authenticated generic contract/lifecycle records |
| Declarative model validation, preview and SQL plans | Yes | Offline; no automatic installation or live projections |
| Model registry, backfill/promotion and generated typed APIs | In progress | Do not attach model JSON to runtime configuration yet |
| Durable real-time streams, joins and aggregates | Planned | No subscription/SSE endpoint yet |
| Remote/devnet/production deployment | Not verified | Current transports are restricted to numeric loopback |

## Choose a starting point

- Start with the [credential-free quickstart](quickstart.md) to learn lifecycle
  replay and simulated visibility without Docker or Canton.
- Use [PostgreSQL](postgresql.md) to try durable ingestion and restart behavior.
- Connect an existing authorized source with the [LocalNet guide](localnet-source.md),
  then configure [API users](authentication.md) and [query your data](querying.md).
- Define a [model](models.md), or follow the [Outcomia example](outcomia.md).

All commands run from the repository root unless stated otherwise. JSON examples
use synthetic values. Party labels such as `A` are fixture identities, not parties
that automatically exist on your Canton participant. A model definition does not
grant access to its inputs.

The [technical runtime contract](../live-runtime.md) and
[verification report](../phase3-verification.md) record the supported behavior and
its evidence. The [roadmap](../roadmap.md) tracks remaining acceptance work.
