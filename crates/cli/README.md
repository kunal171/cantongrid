# cantongrid-cli

The cantongrid command-line executable for fixture replay and the source/indexing operations supported by this release.

## Usage

```bash
cargo install cantongrid-cli --version 0.3.0-alpha.3 --locked
cantongrid --help
cantongrid init ./my-demo
cantongrid fixture run ./my-demo/cantongrid.json --party A
```

This runs a synthetic in-memory fixture. `--party` simulates grants and does not authenticate a live user.

## Release scope

This is the historical 0.3.0-alpha.3 release, packaged for crates.io. Packaging adds
crate-local assets and metadata; the original source release remains unchanged.
Prefer the latest release for new integrations. APIs are alpha and may change.

[Versioned repository and setup](https://github.com/kunal171/cantongrid/tree/b64d7eee9b5b2ddaca92c70eed6bfba57023a86c) · [Current developer guide](https://shady-3.gitbook.io/cantongrid/)

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
