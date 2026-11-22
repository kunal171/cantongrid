# cantongrid-postgres

Restricted PostgreSQL connections and redacted database errors for CantonGrid.

## Usage

```toml
[dependencies]
cantongrid-postgres = "=0.4.0-alpha.4"
```

```rust
use cantongrid_postgres::connect_env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
// The operator sets this variable to an explicit loopback connection string.
let mut database = connect_env("CANTONGRID_DESTINATION_URL")?;
let row = database.query_one("SELECT 1::int4", &[])?;
assert_eq!(row.get::<_, i32>(0), 1);
Ok(())
}
```

Connections require an explicit user and database, and a loopback IP address.
This release uses local plaintext transport; remote/TLS connections are not
supported. Errors redact connection details. This crate does not run migrations
or make source writes; use separate source-reader and owned-destination roles.

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Supported profile: the tested live profile is loopback-only, with authorized node-local
PQS data and snapshot-forward partial history. Model declarations do not grant
source or caller access. See the guide for setup, limits and the LocalNet PoC.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
