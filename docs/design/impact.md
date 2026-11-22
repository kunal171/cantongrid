# M6: upgrade impact analysis

Extend `cantongrid-sync`; do not introduce a second metadata compiler. All public
comparisons verify both DARs and validate the bounded model catalog first.

## Report contract

Add a versioned impact section to the existing compatibility report. For each
model, list projected-field reads, resolved old/new LF semantics and exact scalar
types, and downstream filter, aggregate, relationship and join dependencies.
Include current/proposed fingerprints, SQL table and generated API identities.
Classify rebuilds as unnecessary, required or blocked. Field removal, optionality,
Numeric scale, enum and ContractId target changes retain the existing fail-closed
gate. Never infer a rename or coerce an unsupported type.

Relationships pin target revisions. If a proposed target revision changes, report
an explicit policy-review blocker rather than silently rewriting the pin. Missing
join targets also block complete bundle generation. Retain useful reports when
blocked; a blocked report cannot produce deployable artifacts.

## Local evaluation

`sync evaluate` accepts explicit, bounded synthetic before/after payload fixtures.
Reuse the model evaluator and M5 diagnostic vocabulary. Emit statuses and whether
projected values agree, never payloads, literals or values. No DAR execution or
ledger simulation is implied. A blocked proposal has no candidate evaluation.

## Boundaries and checks

No new dependencies or database migration. Planning, evaluation and bundle
creation cannot connect to a ledger or database. Rebuild instructions require
explicit registration, installation and source-authorized activation; they never
reset checkpoints, delete history, alter grants, upload or vet a DAR.

Test deterministic dependency/report ordering, unchanged and unselected changes,
selected type changes, relationship pin drift, missing targets, rejected bundles,
fixture limits and redaction. Verify public CLI commands using checked-in DAR
fixtures where available. Merge M5/M6 only after repository CI passes.
