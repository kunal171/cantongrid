# cantongrid-ledger

This README describes the 0.4.0-alpha.7 source checkout (unreleased).
Current Canton Ledger API user and party-grant verification for CantonGrid.

## Usage

```toml
[dependencies]
cantongrid-ledger = "=0.4.0-alpha.7"
```

Use `GrantChecker` with the configured source scope and wrap it in
`SourceAuthorization`. Refresh authorization from an explicitly named
`CANTONGRID_*` credential variable before reading; `lease()` returns a borrowed
lease only after a successful check. Failed renewal discards previous success.

This crate verifies the source user's current party grants. API callers require
separate authentication and grant checks. Only the configured loopback LocalNet
JSON Ledger API profile is supported in this release.

See the [source setup](https://github.com/kunal171/cantongrid/blob/main/docs/guide/localnet-source.md)
for endpoint, user and scope configuration.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
