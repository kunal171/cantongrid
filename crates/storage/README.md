# cantongrid-storage

This README describes the 0.5.0-alpha.1 source checkout (unreleased).
Owned PostgreSQL lifecycle persistence, checkpoints and typed model projections for CantonGrid.

## Usage

```toml
[dependencies]
cantongrid-storage = "=0.5.0-alpha.1"
```

`initialize_env` applies destination migrations explicitly. `Store` manages
lifecycle history, durable checkpoints and model projections. Writes and cursor
advancement share transactions so replay can recover without duplicating
already committed deliveries.

With the CLI and an owned destination configured:

```bash
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture index examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture status examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
```

Run fixture commands from a repository checkout. Never point destination
migrations at an upstream PQS database. Typed model registration, installation,
binding and staged rebuilds are separate explicit operations.
Version 0.4.0-alpha.4 also supports private exercise receipts through
`Store::apply_exercise_page`; migration 009 commits their journal/outbox with
the ordinary lifecycle checkpoint. See the repository's
[receipt profile](https://github.com/kunal171/cantongrid/blob/main/docs/design/exercise-ingestion.md).
See the [PostgreSQL quickstart](https://github.com/kunal171/cantongrid/blob/main/docs/guide/quickstart.md).

Opt-in `Store::apply_lineage_page` also commits typed creation evidence and
proven direct edges atomically with the checkpoint (migration 010). Lineage reads
require both indexed endpoints and all proof dependencies to be visible.

Version alpha.4 also supports versioned declared relationships over current model
projections, with independently authorized endpoints. See the
[relationship guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/relationships.md).

Version alpha.4 also includes explicit typed inner joins over declared relationships,
with selected columns and independent input authorization. See the
[join guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/joins.md).

Source alpha.5 adds the [read-only developer inspector](https://github.com/kunal171/cantongrid/blob/main/docs/guide/inspector.md):
authorized transaction/projection evidence and sanitized local diagnostics.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.

Alpha.6 adds bounded `Store::read_as_of` canonical history and private lifecycle
subscriptions, random receipt cursors, monotonic ACKs and authorized outbox
retention. Migration 011 is explicit; canonical events remain retained. Back up
before migration. See the repository delivery and recovery guides for limits.

Alpha.7 source adds `Store::bootstrap_subscription` and `Store::read_bootstrap`:
a private fixed canonical baseline is paired with a durable feed in one writer-locked
transaction. Explicit migration 012 adds its private metadata. Use matching API
binaries and the repository bootstrap guide; use the matching release.

The unreleased 0.5 source adds `Store::read_reconciliation` for exact per-contract
checks over one complete retained indexed prefix and active model revision.
It requires complete trusted caller/source scope and verifies native projection
consistency. Reports contain identities/counts, not contract IDs or financial
values; they do not certify asset units or economic correctness. See the
[reconciliation guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/reconciliation.md).
