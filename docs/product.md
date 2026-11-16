# Product contract

## Purpose

CantonGrid is a general-purpose indexer for authorized Daml contracts on Canton.
It turns selected contract data and declared processing rules into managed
PostgreSQL projections, authenticated GraphQL/REST queries and resumable change
streams. It is a standalone product; Outcomia is its first working example.

The core must not contain Outcomia template names, market accounting, choices,
trade classifications or resolvers. Those belong in an example's verified schema
bindings and processing definitions. Another Daml application must be supported
by changing configuration and metadata, without changing engine code.

## Developer contract

A developer supplies:

1. What: package/template selections, contract/event selections and fields.
2. How: supported filters, mappings, derived fields, joins, grouping and aggregates,
   with explicit output types and domain semantics where needed.

The platform supplies the supported mechanics: discovery, source reads,
normalization, active-state/lifecycle maintenance, managed tables/views and
indexes, versioned migrations, replay, generated APIs, durable change delivery,
authorization enforcement and observability.

No handwritten connector, SQL migration, API resolver or stream handler should
be required for an application using the supported processing vocabulary.
Unsupported rules must fail validation with a useful explanation. General-purpose
does not mean arbitrary code or automatic understanding of every Daml model.

A basic selection should produce a useful canonical contract/event API even
before the developer defines a custom projection. Typed output requires verified
package metadata or explicitly declared projection types; sample JSON alone is
not evidence of a Daml schema.

## Operator responsibilities

Deployment setup is separate from application processing:

- Supply authorized source access and a permitted party allowlist.
- Supply the platform PostgreSQL destination and environment-based secrets.
- Configure API principals and their party subsets, operational access and retention.
- Approve breaking schema/rebuild transitions when required by the migration policy.

The platform scaffolds and validates these settings. It does not grant ledger
permissions, obtain credentials, provision parties or infer authorization from a
caller-supplied party name. Processing definitions cannot override access policy.

## First release

- Daml contracts only; Outcomia first, plus a second unrelated fixture model.
- LocalNet is the intended first real environment. Its availability and compatible
  PQS/SDK versions still need verification. Devnet infrastructure has been requested.
- PQS is the preferred initial source candidate. Its read-only interfaces must be
  verified before implementation; direct Ledger API support is conditional on
  verified need, not an automatic second connector commitment.
- One organization, with separately scoped API tokens for permitted party subsets.
- Active contracts and supported lifecycle history, with exact values, provenance,
  source-local ordering, completeness and freshness information.
- Declarative processing, generated authenticated REST/GraphQL access, and durable
  resumable server-sent event streams, delivered in successive implementation phases.
- Local fixture demos remain useful without a participant, funds or credentials.

Typing and advanced processing are capability-gated. Realtime means committed
indexed changes with recovery semantics, not instant access to every private
network event. Data hidden by authorization or missing upstream history cannot
be reconstructed by the platform.

## Outside this release

Public Scan ingestion; token-standard asset adapters; validator operations;
governance; hosted multi-tenancy; arbitrary SQL or code plugins; AI-controlled
runtime transformations; transaction submission; MainNet deployment; and claims
of universal Canton history or causal ordering across independent sources.

Future source extensions may use the same provenance and policy boundaries.
They must not displace the Daml-first product or complicate its first release.

## Release claims

Every capability must be labelled planned, fixture-tested, LocalNet-tested or
verified against an authorized live source. Passing fixtures is not live Canton
integration. A working Outcomia example is mandatory; the engine must also pass
the second-model configuration-only test.

The name CantonGrid is provisional. Apache-2.0 is the license target, with the
license declaration and dependency review still pending before public release.
