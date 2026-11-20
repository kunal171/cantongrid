# cantongrid-runtime

Authenticated CantonGrid indexing workers and REST/GraphQL query services.

## Usage

```toml
[dependencies]
cantongrid-runtime = "=0.4.0-alpha.1"
```

The library exposes `index`, `serve`, `bind_model` and bounded `rebuild`
operations. It combines source authorization, PQS paging, owned PostgreSQL
persistence and caller-scoped REST/GraphQL. Credentials are supplied through
explicit operator environment variables referenced by runtime configuration.

Most applications should run the equivalent CLI commands:

```bash
cantongrid index runtime.json --once
cantongrid serve runtime.json
```

These require a prepared runtime configuration and databases; they are not a
standalone fixture. Follow the [LocalNet guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/localnet-source.md)
and [API guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/api.md).

Authenticated model queries are available through `/models/graphql`. Real-time
stream delivery is planned and is not provided by this release.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
