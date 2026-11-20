# cantongrid-storage

Owned PostgreSQL lifecycle persistence, checkpoints and typed model projections for CantonGrid.

## Usage

```toml
[dependencies]
cantongrid-storage = "=0.4.0-alpha.1"
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
See the [PostgreSQL quickstart](https://github.com/kunal171/cantongrid/blob/main/docs/guide/quickstart.md).

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
