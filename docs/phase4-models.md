# Phase 4: declarative model contract

Status: compiler/evaluator/SQL planning and offline CLI implemented on
`feat/declarative-models`, targeting `0.3.0-alpha.1`. Live materialization and
generated APIs are not wired yet; Phase 4 acceptance is incomplete.
Phase 3's source identity, current source/caller checks and partial-history profile
remain prerequisites. Models never grant access to their inputs.

## First model vocabulary

A version-1 model document declares 1–16 named models. Each model pins one
exact template, a positive revision, up to 32 named output fields and an optional
output-field equality filter. Field expressions read bounded object paths,
provide typed literals, or perform checked add/subtract/multiply operations.
No arbitrary SQL, code, network I/O or Daml business-rule inference is accepted.

Supported output types: Text, Party, Bool, Int64, Numeric with declared scale
0–37, and one optional layer around a scalar. Source Int64/Numeric inputs must be
strings. Missing/null optional inputs become None; missing required inputs fail.
JSON numbers are not silently interpreted as financial values. Decimal values
are normalized to declared scale without rounding. Arithmetic must stay within
the declared Daml precision/Int64 bounds; overflow fails the whole evaluation.
Add/subtract require identical scalar types. Multiply supports Int64×Int64 and
Numeric×Int64 (either order), retaining Numeric scale. Optional arithmetic and
Numeric×Numeric are initially unsupported rather than guessed.

Expression depth is at most 16, with at most 256 nodes per model; paths have at
most 16 identifier segments. Identifiers are bounded ASCII names, exclude system
metadata/root names and cannot introduce SQL/GraphQL syntax. Definitions have a
64-KiB input limit. Errors and Debug output must not expose input values/literals.

## Planning and runtime boundaries

The compiler validates names, versions, paths, types, arithmetic and filters
before creating a canonical definition/fingerprint. Generated SQL uses validated,
quoted field identifiers and definition-derived table names, never supplied SQL.
Tables preserve Int64 as bigint and Numeric as numeric(38, scale); public exact
scalars retain strings. Model revisions are immutable: a changed definition needs
a new revision and an explicit rebuild/promotion plan.

A single-contract projection inherits its canonical parent's visibility and
lifecycle. Materialized reads must join current active canonical inputs and apply
source∩caller grants before counts/pages. Archive retracts the corresponding
active projection in the same transaction as state/checkpoint/outbox. No model
row may widen the source policy. Future joins require AND across every input,
not the union of parties.

Model definitions/versions belong to a separate registry from the raw source
plan: changing an output model must not silently reuse stale model cursors or
force an unrelated source re-snapshot. Backfill/replay and promotion must be
atomic or explicitly staged, preserve the previous usable version until success,
and reject unsupported/oversized rebuilds. Generated typed GraphQL and managed
storage will use the same compiled definition; developers do not handwrite
application-specific Rust, migrations or resolvers.

## Evidence and limits

The first implementations must prove deterministic evaluation/planning with both
Outcomia and unrelated Visibility definitions, exact maximum Int64/large Numeric
values, overflow/type/null rejection, filters and malformed definitions. Storage,
API and lifecycle integration are separate acceptance checks and must not be
reported complete from compiler tests. No new external runtime dependency is
needed for this initial compiler/evaluator/planner.


## Implemented commands and configuration

```bash
cantongrid models validate examples/models/outcomia.json
cantongrid models plan examples/models/visibility.json
cantongrid models preview examples/models/visibility.json records /path/to/payload.json
```

These commands require no source connection or API credential. Validation emits
only plan metadata; planning emits reviewable generated DDL, without executing
it; preview deliberately reads an operator-supplied local payload file and emits
typed projected values. Preview is not a server query or an authorization fallback.
It does not validate that the input came from the declared template.

[Outcomia](../examples/models/outcomia.json) projects verified BinaryMarket paths
including exact Int64 lot, price tick and `priceTickBps = priceTick * 10000`.
[Visibility](../examples/models/visibility.json) projects owner/amount/sequence,
optional note and state, with an equality filter for Ready. Both pin real reviewed
package identifiers; unit-test payloads are explicitly synthetic. This is not a
new live-model demonstration or proof of accounting semantics.

A field declaration looks like:

```json
{
  "op": "field",
  "path": ["terms", "priceTick"],
  "as": {"type": "numeric", "scale": 10}
}
```

Definitions are strict JSON: duplicate/unknown fields, unsafe names, unsupported
operators/types and excessive expression depth are rejected. Filters run after
projection/validation; invalid values do not disappear because a filter is false.
Outputs use the core's explicit typed-value encoding. Numeric source normalization
may add trailing zeros to the declared scale but never drops digits or rounds.
Projected text, including optional text, literals and filter values, cannot contain
NUL because PostgreSQL text/jsonb cannot represent it. Such values fail validation;
they are never stripped. Both evaluation and direct typed-row encoding enforce the
1-MiB serialized projection limit.

Generated SQL targets `cantongrid_models`, quotes validated names, derives table
names from definition fingerprints, uses exact bigint/numeric columns and links
rows to canonical parent contracts. The schema/registry installer and automated
migration/promotion mechanism are not implemented yet. The owned-Postgres test
creates the generated table inside a rollback-only transaction to verify types
and exact round trips; it does not install a live model.

Next integration: versioned model registry/backfill/promotion, guarded model
materialization/retraction inside canonical ingestion transactions, authorized
model counts/pages and generated typed GraphQL. Aggregates/joins require separate
transitive privacy and lifecycle acceptance; streams remain Phase 5.
