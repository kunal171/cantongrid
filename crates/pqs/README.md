# cantongrid-pqs

Read-only authorized Canton Participant Query Store snapshot and update paging.

## Usage

```toml
[dependencies]
cantongrid-pqs = "=0.4.0-alpha.4"
```

Use `inspect_env` to check a dedicated PQS reader's schema and retained boundary.
`page_env` reads bounded authorized snapshot/update pages; `reconcile_env` checks
a retained historical interval. These calls require the configured database
role and, for authorized paging, current source authorization.

The supported adapter targets PQS SQL schema 041. A readable SQL connection does
not grant permission to serve contract data. Snapshot-forward ingestion reports
partial history; it cannot recreate events already pruned upstream.

For an operator inspection with the CLI:

```bash
cantongrid source inspect-pqs --database-env CANTONGRID_PQS_URL
```

See the [source guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/localnet-source.md)
for role setup and supported coverage.

For an explicitly enabled lineage plan, `lineage_page_env` reads typed creation
provenance together with exercise receipts in one bounded read-only snapshot.
Transient metadata does not change create/archive contract materialization.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
