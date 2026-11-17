# Phase 4: declarative model contract

Status: implementation started on `feat/declarative-models`. This is the next
release's bounded model slice, not a claim that Phase 4 acceptance is complete.
Phase 3's source identity, current source/caller checks and partial-history profile
remain prerequisites. Models never grant access to their inputs.

## First model vocabulary

A version-1 model document declares up to 16 named models. Each model pins one
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
