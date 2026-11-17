# CantonGrid core

The first Rust library contains source-neutral visibility policies. It has no
network, storage, application template or serialization dependencies.

For one input record, an authorized caller may match any verified witness party.
For a result derived from several inputs, the caller must match every input's
visibility clause. Joining policies preserves those requirements and rejects
mixed source/generation bindings. Missing evidence is rejected; identities remain
opaque and are redacted in Debug output.

`CallerGrants::from_verified` assumes authentication and current grant resolution
have already happened. The pure policy check is not an authentication provider,
source-authorization lease, HTTP middleware or data-serving implementation.

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
SLA. Network grant retrieval, credential verification, lease renewal, broadcast
invalidation, ingestion and API integration remain to be built. Verified snapshots
and the monotonic clock must come from trusted providers, never client input.
