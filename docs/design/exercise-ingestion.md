# Internal exercise receipts (M1)

Status: implemented in the source checkout; pinned LocalNet verification passed.
Included in 0.4.0-alpha.3. Branch builds on M0 commit `b4f8da5`.
Source decisions and scoped LocalNet evidence: [M0](exercise-source.md).

## Contract

- Explicit tree-mode opt-in changes the durable source binding. Existing
  create/archive configurations retain their cursor and behavior. Operators must
  use a new generation and a separately prepared source database when switching source mode; SQL catalogs cannot
  prove the exporter's running transaction shape.
- Initial contract snapshot remains partial history. Exercises start after that
  snapshot boundary; no historical receipt is invented for an active contract.
- Forward tree-mode pages contain one complete selected transaction: lifecycle
  events plus exercises, at most 64 combined receipts and 1 MiB serialized data.
  Reject oversize input without changing destination state or checkpoint.
- Versioned receipts retain participant-local offset/node ID, transaction update
  ID, executed template/package identity, contract/choice, consuming flag,
  argument/result, controllers, exercise witnesses, time and descendant bound.
  Unknown bounds remain unknown. No parent edge is inferred.
- Exercises do not mutate contract/model state. Existing archives retain that
  responsibility. Exercise evidence can exist without a prior indexed create;
  a partial projection or divulged event is not a missing-create repair.
- Migration 009 adds private exercise receipts, delivery evidence and an internal
  outbox. They commit in the existing writer transaction with lifecycle events,
  projections, journal and compare-and-swap checkpoint. No new runtime dependency.
- Receipt keys are `(stream, offset, node_id)`. Replayed identical delivery is a
  no-op; changed receipt content or a reused identity rejects the whole delivery.
  Current exact source grants and lease expiry still gate reads and commits.

## Configuration and rollout

Prepare a schema041 exporter with `pipeline.datasource = TransactionTreeStream`,
exact party filters, `numericAsString`/`int64AsString`, and a SELECT-only reader.
Add this field to the normal runtime JSON:

```json
"exercise_source": "pqs041_ledger_effects"
```

Omitting the field retains legacy create/archive polling. Any other value is
rejected. The configured profile is an operator assertion; an empty exercise
table cannot establish exporter shape. Changing the profile in an existing
generation conflicts with its durable definition. Use a new explicit generation.

Before opening the new binary against an existing destination, run:

```bash
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid index path/to/runtime.json --once
```

Initialization explicitly upgrades intact 001–008 destinations to 009. Previous
migration bytes stay unchanged. Old binaries reject 009; rollback requires the
previous binary and a pre-upgrade destination backup. Do not edit migration
history or drop receipt tables to force a downgrade.

`page_size` still controls snapshot pages. Forward receipt polling selects one
complete supported transaction, capped at 64 combined lifecycle/exercise records
and 1 MiB. Missing prior creates are allowed for evidence targets; they never
authorize an archive or repair contract state. Receipt/outbox tables are private;
existing contract/event APIs expose no exercise data.

Package handoff for receipt streams, interface-choice evidence, redacted payloads,
reassignments, general immediate-parent proof and pre-snapshot history remain
unsupported. Detected redaction or ambiguous choice identity stops the page.
Package handoff rejects receipt streams before publication.

## Checks

Core validation: exact values, required identities, witness/controller separation,
unknown bounds, payload limits and unsupported wire versions. Source: same-offset
ties, overlapping restart, partial visibility, pruning/regression, oversized
transactions and SELECT-only access. Destination: migration upgrade/drift/ACLs,
duplicate/conflicting receipts, stale workers, expired lease and rollback after
receipt staging. Run the existing lifecycle/model suites to detect regressions.

Read-only source tests compare the retained M0 window across four party scopes;
their Rust leases are synthetic. The real worker test separately uses current
Ledger authorization, exporter health and the previously approved pair/nested
controls: four receipts/outbox entries, three consuming exercises, two unindexed
targets, one lifecycle archive, and no extra receipt on an idle restart. See
[reproduction](../../examples/exercises/README.md) and [verification](../verification.md).

Use retained M0 fixtures for read-only LocalNet verification when possible. New
ledger writes or provisioning beyond the existing approved experiment require
separate authorization. Record mock, PostgreSQL and LocalNet results separately.
M2 graph APIs, joins, full history and public streams remain deferred.
