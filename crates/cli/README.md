# cantongrid-cli

This README describes CantonGrid 0.4.0-alpha.7.
CantonGrid CLI for authorized Daml contract indexing, model generation and query services.

The executable is named `cantongrid`. Run `cantongrid --help` for commands.

## Install and try it

Requires Rust 1.100.0 or newer. The package installs the `cantongrid` executable.
Install the matching prerelease below, or build its source tag with
`cargo build -p cantongrid-cli --locked`.

```bash
cargo install cantongrid-cli --version 0.4.0-alpha.7 --locked
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
