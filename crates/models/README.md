# cantongrid-models

Declarative Daml contract projections, exact calculations, SQL and client type generation.

## Usage

```toml
[dependencies]
cantongrid-models = "=0.4.0-alpha.5"
```

The library exposes `Catalog`, `Model`, exact typed expressions, SQL plans and
validated query options. Parsing and evaluating a model does not connect to a
source, install a table or authorize a caller.

With `cantongrid-cli` installed, run these commands from the repository checkout:

```bash
cantongrid models validate examples/models/visibility.json
cantongrid models preview examples/models/visibility.json records examples/models/visibility-payload.json
cantongrid models plan examples/models/visibility.json
cantongrid models typescript examples/models/visibility.json
cantongrid models graphql examples/models/visibility.json
```

Numeric and Int64 values retain exact string encodings. SQL installation,
source binding and active revision changes are explicit operator actions.
See the [model guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/models.md)
for a complete definition and lifecycle.

Version alpha.4 also supports versioned declared relationships over current model
projections, with independently authorized endpoints. See the
[relationship guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/relationships.md).

Version alpha.4 also includes explicit typed inner joins over declared relationships,
with selected columns and independent input authorization. See the
[join guide](https://github.com/kunal171/cantongrid/blob/main/docs/guide/joins.md).

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
