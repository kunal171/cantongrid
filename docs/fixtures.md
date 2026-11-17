# Rust fixture milestone

Work in progress: strict project configuration is implemented in `core::config`.
The runner and fixture commands follow in this milestone. This is a synthetic,
credential-free boundary; it does not assert live PQS or Ledger API compatibility.

Project JSON uses explicit `version`, `model_version` and `payload_schema_version`
(all currently 1). A source declares the `fixture` adapter, stable source `id`,
`network`, positive authorization `generation`, unique permitted `parties` and
fixture path. Selections use exact package ID/module/entity identifiers, both
create/archive event kinds and complete history. Partial histories, unknown
fields, live adapters and processing operators fail validation. No credentials
belong in fixture configuration. Paths will resolve relative to the project file.

Source capabilities describe supported lifecycle/history operations; they are not
inferred from a successful empty response. The fixture profile supports create,
archive and complete synthetic history only. Source/network/generation bind
visibility and progress. Versions must match before ingestion; a different model
requires a separate rebuild, never reuse of old state.

Serde and serde_json are the first dependencies, locked in Cargo.lock. Typed
configuration rejects unknown fields. Diagnostics deliberately suppress parser
messages that could include field names, credentials or payload values. Runtime
and API authorization still require trusted grant providers; fixture party names
are test inputs only.

## Exact fixture values

The Rust `value` module accepts explicit tagged Text, Bool, Int64, Numeric,
Optional, Enum, List and Record values. Int64 and decimals use canonical strings;
Numeric declares its scale (0–37) and enforces precision 38, following the
[Daml Numeric definition](https://archived.docs.digitalasset.com/build/3.5/reference/daml/stdlib/Prelude.html).
No arithmetic or inferred Daml schema validation is implemented here. Unsupported
kinds, out-of-range values and excessive nesting fail validation. Debug output
redacts payloads. Live encodings must be normalized by a verified adapter;
fixture encodings are not a claim about the Ledger API's wire format.

## Lifecycle reference engine

`lifecycle::Engine` holds a validated source/network/generation context and an
in-memory canonical contract/history store. It applies each opaque-position batch
atomically by staging a copy; it never orders positions lexically or numerically.
An identical batch redelivery is a no-op and cannot regress the checkpoint.
Conflicting positions/event IDs, unknown or repeated archives, contract ID reuse,
invalid exact values and missing/out-of-scope witnesses fail without publishing
partial state. Exact template selections do not infer archives from absent rows.

Active and history reads require caller grants and a current source lease.
Archive history retains both original-create and archive visibility dependencies.
Event provenance retains source context, position, event/contract/template IDs,
witnesses, observation time and optional ledger time. Times are explicit integer
microseconds in this reference format, not inferred from ingestion wall time.

This engine assumes ordered, complete create/archive input from origin. It rejects
an archive with no selected create rather than inventing missing history. Snapshots,
partial/pruned history, exercises and reassignments are not supported in this
slice. State/checkpoints are process memory; owned PostgreSQL transactions,
checkpoint recovery and outbox delivery remain Phase 3 work.
