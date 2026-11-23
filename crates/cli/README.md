# cantongrid-cli

This README describes the 0.5.0-alpha.1 prerelease.
CantonGrid CLI for authorized Daml contract indexing, model generation and query services.

The executable is named `cantongrid`. Run `cantongrid --help` for commands.

## Install and try it

Requires Rust 1.100.0 or newer. The package installs the `cantongrid` executable.
The example below installs stable 0.4.0. Build this unreleased checkout with
`cargo build -p cantongrid-cli --locked`.

```bash
cargo install cantongrid-cli --version 0.5.0-alpha.1 --locked
cantongrid --help
cantongrid init ./my-demo
cantongrid validate ./my-demo/cantongrid.json
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

This example runs synthetic events in memory without Canton or PostgreSQL.
`--party` simulates fixture grants; it does not authenticate a live user.

For a configured live project, `cantongrid index runtime.json` ingests contracts
and `cantongrid serve runtime.json` serves authenticated REST/GraphQL.
`cantongrid models` validates projections and generates SQL, TypeScript and
GraphQL definitions. Source credentials and database setup are separate steps;
follow the [LocalNet PoC](https://github.com/kunal171/cantongrid/blob/main/docs/guide/poc.md).

Version alpha.5 adds the [read-only developer inspector](https://github.com/kunal171/cantongrid/blob/main/docs/guide/inspector.md):
authorized transaction/projection evidence and sanitized local diagnostics.
`cantongrid sync plan` reports upgrade dependencies and rebuild/API impact;
`sync summary` and `sync evaluate` provide offline review and redacted sample checks.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.

Unreleased 0.5 source adds `reconcile validate <policy.json>` and
`reconcile evaluate <policy.json> <fixture.json>`. These commands read supplied
local files without source credentials or database access. Diagnostic success
never means a verified financial all-clear. See the repository reconciliation
examples and guide for exact arithmetic, limits and evidence semantics.

`cantongrid reconcile preflight <policy.json> <catalog.json> <namespace> <model>`
checks projected field compatibility and exact definition identities offline.
It does not install/bind models or check indexed data.
