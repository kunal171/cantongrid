# Developer processing model

Status: intended contract. Configuration syntax, expression grammar and generated
API names will be finalized with validated examples during implementation.

## What developers supply

A project has three concerns: operator connection/access settings, data selections,
and processing models. Credentials stay outside committed model definitions.
Developers should not write ingestion loops, SQL migrations, Rust adapters or
GraphQL resolvers for a supported model.

| Input | Developer decision | Platform responsibility |
| --- | --- | --- |
| Selection | Packages/templates, event kinds and required fields. | Validate against available metadata and authorized source capabilities. |
| Projection | Output fields and their types, filters and mappings. | Validate, materialize and expose queryable results. |
| Derived data | Exact calculations, grouping and supported aggregates. | Maintain deterministic state through updates, replay and rebuild. |
| Relationships | Explicit keys, supported joins and lifecycle behavior. | Track dependencies and enforce access to all contributing inputs. |
| Exposure | Which declared models are queryable or streamed. | Generate consistent authenticated APIs and scoped cursors. |

The first generic slice does not require a custom projection. More advanced
operators arrive in phases and are rejected clearly until supported. Arbitrary
user SQL, executable plugins and arbitrary Daml business-rule inference are not
part of the first release.

## Illustrative model

This is pseudocode, not valid CLI configuration or a claim about Outcomia's real
Daml templates:

```text
select authorized contracts of Demo:Asset
model active_assets:
  key = source identity + contract ID
  owner: Party = payload.owner
  amount: Numeric = payload.amount
  include while contract is active
  expose query and lifecycle changes
```

CantonGrid would validate the referenced template and types, generate managed
storage and query fields, remove an active row on a verified archive, and emit a
durable change. A grouped owner total would additionally declare its grouping
key and exact sum. Its authorization depends on every contributing record;
a global total cannot be returned to a caller who can see only some inputs.

Outcomia's example must use actual verified packages and domain mappings. Market,
trade or balance fields must not be guessed from Solana structures or template
names. A second unrelated Daml example must use the same public model interface.

## Determinism and lifecycle

Models must declare stable keys and supported input/output types. Processing must
be repeatable from the same authorized input and versioned definition, independent
of wall-clock time or network side effects. Distinguish creation, archival and
other supported lifecycle observations; do not assume every exercised choice
archives a contract. Never infer an archive from absence in a partial snapshot.

Dependencies determine retraction and recomputation on archival, correction or
rebuild. Preserve exact arithmetic; define null, optional, enum, timestamp and
rounding semantics before exposing operators that use them. Unsupported types or
ambiguous mappings must produce actionable errors rather than guessed values.

## Generated behavior and evolution

The platform owns planning, storage, migration proposals, checkpointing, API
schema generation and outbox writes. Developers own the meaning of their model,
including accounting definitions. Authenticated source access and API exposure
policy remain explicit operator responsibilities.

A definition change creates a versioned plan. Validate compatibility before
applying it, rebuild when necessary, and keep the previous usable version until
promotion. Query and stream cursors must identify their schema/policy context;
replaying a changed definition must not mix incompatible results.

Acceptance requires Outcomia and the unrelated example to work without edits to
core Rust, SQL migrations or generated resolvers. This is the test of the product
promise, not merely a demonstration of configurable template names.
