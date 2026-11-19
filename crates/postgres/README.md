# cantongrid-postgres

Explicit loopback PostgreSQL connections with redacted errors. Use separate reader and destination roles; this crate does not apply source migrations.

## Usage

```toml
[dependencies]
cantongrid-postgres = "=0.3.0-alpha.3"
```

[Library modules and examples](https://github.com/kunal171/cantongrid/tree/b64d7eee9b5b2ddaca92c70eed6bfba57023a86c/crates/postgres/src) describe the API available in this version.

## Release scope

This is the historical 0.3.0-alpha.3 release, packaged for crates.io. Packaging adds
crate-local assets and metadata; the original source release remains unchanged.
Prefer the latest release for new integrations. APIs are alpha and may change.

[Versioned repository and setup](https://github.com/kunal171/cantongrid/tree/b64d7eee9b5b2ddaca92c70eed6bfba57023a86c) · [Current developer guide](https://shady-3.gitbook.io/cantongrid/)

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
