# Rust configuration and fixture pipeline

Status: implemented and fixture-tested. See the [CLI workflow](cli.md) and
[synthetic examples](../examples/fixtures/README.md). No live connector or database
persistence is included in this milestone.

## Project configuration

The committed format is JSON version 1, demonstrated by both example projects.
Project, model and payload schema versions are explicit and currently must be 1.
The only supported adapter is `fixture`; source identity, network, positive scope
generation, unique source parties and fixture path are required. Exact package
IDs must be 64 lowercase hexadecimal characters; module/entity names are explicit.
Selections require both create/archive and complete history. Empty/wildcard or
duplicate selections are rejected. Models/operators, credential settings and live
adapters are rejected until their implementation phases rather than silently
ignored. Source capability validation is a plan gate, not a claim that a live
source has those guarantees.

Errors omit raw input, identifiers and credentials. Unknown fields fail closed
using [Serde strict containers](https://serde.rs/container-attrs.html). JSON is
parsed through a duplicate-key/float-rejecting boundary before typed decoding.

Source capabilities are not inferred from a successful empty response. The
fixture profile supports create, archive and complete synthetic history only.
Source/network/generation bind visibility and progress; positions are meaningful
only in that context. A changed model requires a separate rebuild. Source paths
resolve relative to the project file. Serde and serde_json are locked in
Cargo.lock; diagnostics suppress raw parser messages. Real source/API grants
still require trusted providers; fixture party names are test inputs only.

## Exact fixture values

The Rust `value` module accepts explicit tagged Text, Party, Bool, Int64, Numeric,
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

The `fixture` module decodes strict versioned JSON and replays batches through the
same lifecycle engine. Duplicate JSON keys and numeric floating-point tokens are
rejected before normalization; raw parser diagnostics are never logged. Fixture
versions and source/network/generation must match the validated project. Only
complete-from-origin input is accepted. A second pass over an existing engine must
apply zero batches. [Both examples](../examples/fixtures/README.md) exercise the
same runner; fixture parties simulate callers and are not an authentication API.
