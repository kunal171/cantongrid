# cantongrid-cli

The cantongrid command-line executable for fixture replay and the source/indexing operations supported by this release.

## Usage

```bash
cargo install cantongrid-cli --version 0.2.0-alpha.1 --locked
cantongrid --help
cantongrid init ./my-demo
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

This runs a synthetic in-memory fixture. `--party` simulates grants and does not authenticate a live user.

## Release scope

This is the historical 0.2.0-alpha.1 release, packaged for crates.io. Packaging adds
crate-local assets and metadata; the original source release remains unchanged.
Prefer the latest release for new integrations. APIs are alpha and may change.

[Versioned repository and setup](https://github.com/kunal171/cantongrid/tree/v0.2.0-alpha.1) · [Current developer guide](https://shady-3.gitbook.io/cantongrid/)

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
