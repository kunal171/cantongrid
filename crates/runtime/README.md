# cantongrid-runtime

This README describes the 0.5.0-alpha.2 prerelease.
Authenticated CantonGrid indexing workers and REST/GraphQL query services.

## Usage

```toml
[dependencies]
cantongrid-runtime = "=0.5.0-alpha.2"
```

The library exposes `index`, `serve`, `bind_model` and bounded `rebuild`
operations. It combines source authorization, PQS paging, owned PostgreSQL
persistence and caller-scoped REST/GraphQL. Credentials are supplied through
explicit operator environment variables referenced by runtime configuration.

Most applications should run the equivalent CLI commands:

```bash
cantongrid index runtime.json --once
cantongrid serve runtime.json
```

These require a prepared runtime configuration and databases; they are not a
standalone fixture. Follow the [LocalNet guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/localnet.md)
and [API guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/api.md).

Authenticated model queries are available through `/models/graphql`. Alpha.6 adds
bounded canonical as-of queries and durable REST lifecycle long polling with
monotonic ACKs and explicit retention gaps. These do not provide model CDC or
an atomic snapshot/feed handoff. See the
[delivery guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/delivery.md).

Opt-in direct lineage is available through `/v1/contracts/{contractId}/lineage`
and `contractLineage` on the generic GraphQL API. Responses contain partial
one-hop evidence, with caller and source checks before delivery. See the
[lineage guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/composition.md#direct-lineage).

Version alpha.4 also supports versioned declared relationships over current model
projections, with independently authorized endpoints. See the
[relationship guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/composition.md#declared-relationships).

Version alpha.4 also includes explicit typed inner joins over declared relationships,
with selected columns and independent input authorization. See the
[join guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/composition.md#typed-inner-joins).

Source alpha.5 adds the [read-only developer inspector](https://github.com/kunal171/cantongrid/blob/main/docs/guide/operations.md#inspector):
authorized transaction/projection evidence and sanitized local diagnostics.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.

Version 0.4.0-alpha.4 also exposes reviewed package upgrade planning and staged
application. See the [upgrade guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/upgrades.md)
for source checks, publication gates, resume/cancel and model rollback.

Alpha.7 adds `POST /v1/subscriptions/bootstrap` and
`GET /v1/subscriptions/{id}/snapshot`. The source offset stays private; snapshot
and feed cursors bind independently to the same authorized subscription. See the
[bootstrap guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/delivery.md#bootstrap)
for migration 012, retries, retention gaps and application transaction requirements.

Version 0.5.0-alpha.2 adds an optional `reconciliation_file` manifest and authenticated
configured reconciliation in REST/GraphQL, with full source/caller scope and final
configuration/revision checks. [Setup and bounds](https://github.com/kunal171/cantongrid/blob/main/docs/guide/reconciliation.md#api-050-alpha2).
