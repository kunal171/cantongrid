# CantonGrid core

The Rust library contains source-neutral configuration, exact fixture values,
visibility/lease decisions, lifecycle state and deterministic replay. It uses
Serde/serde_json at the document boundary, with no network or database access.
See [fixture contracts](https://github.com/kunal171/cantongrid/blob/main/docs/fixtures.md) and the [CLI](https://github.com/kunal171/cantongrid/blob/main/docs/cli.md).

For one input record, an authorized caller may match any verified witness party.
For a result derived from several inputs, the caller must match every input's
visibility clause. Joining policies preserves those requirements and rejects
mixed source/generation bindings. Missing evidence is rejected; identities remain
opaque and are redacted in Debug output.

`CallerGrants::from_verified` assumes authentication and current grant resolution
have already happened. The pure policy check is not an authentication provider,
source-authorization lease, HTTP middleware or data-serving implementation.

## Usage

```toml
[dependencies]
cantongrid-core = "=0.4.0-alpha.3"
```

```rust
use cantongrid_core::value::{Decimal, Value};

// Scale 2 preserves the exact amount, including the trailing zero.
let amount = Value::Numeric(Decimal { scale: 2, decimal: "1.20".into() });
assert!(amount.validate().is_ok());
```

[Repository](https://github.com/kunal171/cantongrid) · [Developer guide](https://shady-3.gitbook.io/cantongrid/)

Checks, using the tested Rust/Cargo 1.100 toolchain:

```bash
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
cargo fmt --all -- --check
```

`authorization::SourceLease` adds a pure decision gate over source grants and the
visibility policy. Leases have a positive lifetime of at most 15 seconds, expire
earlier when the verified credential does, and can be invalidated immediately.
They are not cloneable/deserializable. Each input requires a party present in
both the caller's current grants and the source's current grants. Source and
generation bindings must match, and the expiry boundary is exclusive.

The 15-second value is an initial core policy limit, not a deployed revocation
SLA. The separate ledger/runtime crates perform network grant retrieval, credential
verification, renewal, ingestion and API integration. Verified snapshots and the
monotonic clock must come from trusted providers, never client input.

Licensed under Apache-2.0; see `LICENSE` and `NOTICE`.
