# Internal exercise receipts (M1)

Status: implementation in progress. Branch builds on M0 commit `b4f8da5`.
Source decisions and scoped LocalNet evidence: [M0](exercise-source.md).

## Contract

- Explicit tree-mode opt-in changes the durable source binding. Existing
  create/archive configurations retain their cursor and behavior. Operators must
  use a new generation/database when switching source mode; SQL catalogs cannot
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

## Checks

Core validation: exact values, required identities, witness/controller separation,
unknown bounds, payload limits and unsupported wire versions. Source: same-offset
ties, overlapping restart, partial visibility, pruning/regression, oversized
transactions and SELECT-only access. Destination: migration upgrade/drift/ACLs,
duplicate/conflicting receipts, stale workers, expired lease and rollback after
receipt staging. Run the existing lifecycle/model suites to detect regressions.

Use retained M0 fixtures for read-only LocalNet verification when possible. New
ledger writes or provisioning beyond the existing approved experiment require
separate authorization. Record mock, PostgreSQL and LocalNet results separately.
M2 graph APIs, joins, full history and public streams remain deferred.
