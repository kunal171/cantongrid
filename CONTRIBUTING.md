# Contributing to CantonGrid

CantonGrid is a general-purpose, authorization-aware Daml contract indexer written
in Rust. Outcomia is its first example. Read [AGENTS.md](AGENTS.md), the
[decisions](docs/decisions.md), [roadmap](docs/roadmap.md) and
[security model](docs/security.md) before changing runtime behavior. The
[Phase 3 handoff](docs/specifications/phase3-v0.3.md) defines the remaining live
source/API acceptance; read it alongside current verification, not as an up-to-date
implementation inventory.

## Start without Canton infrastructure

Requirements: Rust/Cargo 1.100.0 (rustfmt and Clippy) and Python 3.11+.

```bash
cargo +1.100.0 fmt --all -- --check
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
cargo +1.100.0 run -p cantongrid-cli -- fixture run examples/fixtures/visibility/cantongrid.json --party A
cargo +1.100.0 run -p cantongrid-cli -- fixture run examples/fixtures/outcomia/cantongrid.json --party A
```

The examples are synthetic. Fixture party flags simulate grants; they do not
create authenticated users. After dependencies are fetched, add `--offline` to
Cargo checks. Keep `Cargo.lock` committed and avoid unrelated dependency updates.

## PostgreSQL tests

Docker Engine and Docker Compose with `up --wait` support are required. This
creates a separate local test destination, not a Canton participant or PQS source:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cargo +1.100.0 test -p cantongrid-storage --locked -- --ignored
cargo +1.100.0 test -p cantongrid-cli --locked -- --ignored
cargo +1.100.0 test -p cantongrid-pqs destination_owner_cannot_be_used_as_a_source_reader --locked -- --ignored
python3 scripts/dev.py storage stop
```

The password above is a public disposable fixture default. Do not reuse it on
shared infrastructure. Stop preserves the owned volume. Do not delete source or
application volumes to make tests pass. The full [development command inventory](scripts/README.md)
separates setup commands from probes that temporarily change fixture grants.

## Code organization

- `crates/core`: configuration, exact values, canonical lifecycle and authorization rules.
- `crates/postgres`: shared connection restrictions and redacted errors.
- `crates/storage`: destination migrations, atomic writes, replay and outbox.
- `crates/pqs`: verified read-only source interfaces and capabilities.
- `crates/cli`: command dispatch; keep source and destination commands separate.
- `examples`: public configurations and synthetic fixture data, with no engine special cases.
- `scripts/devtools`: local development/research helpers, never the product runtime.

Keep APIs and processing in their own modules as they arrive. Prefer existing
interfaces and test doubles over app-specific branches or another general client.

## Changes and evidence

Use small commits around one concern. Include a regression test for correctness,
authorization, recovery or precision changes. Report relevant checks actually run;
label evidence fixture-tested, owned-Postgres-tested or LocalNet-tested. A successful
fixture is not a live integration test. Record unsupported behavior explicitly.

Preserve these invariants:

- Source access and API-user access are independent; filters/party strings never grant permission.
- Missing/expired grants fail closed, including counts, errors, cursors and caches.
- Numeric/Int64 values remain exact; never route accounting through floating point.
- State, history, checkpoint and outbox publish atomically; duplicates must not add effects.
- Source positions are local to their source/scope/generation, never global block heights.
- Never infer an archive from absence in a snapshot or fabricate visibility/history.
- Applied migrations are immutable; add a new version rather than edit a recorded checksum.

No live credentials, JWTs, party IDs or private payloads belong in issues, fixtures,
logs or screenshots. Keep local configuration under ignored paths. Use public
synthetic values in reproductions and [private reporting](SECURITY.md) for vulnerabilities.
Do not upload DARs, create parties, submit transactions, modify upstream PQS tables,
or deploy against somebody else's infrastructure as part of a routine test.

For source failures, first classify configuration/processing versus documented
source behavior versus a reproducible upstream defect. Capture minimal nonsecret
controls and exact versions before proposing an upstream contribution. See
[triage](docs/upstream-triage.md).

## Pull requests and releases

Open a PR against `main` from a feature or release branch. Explain the problem,
behavior changed, validation and material limits. Point out configuration/CLI,
schema, authorization or migration compatibility changes. Existing behavior must
keep passing; do not silence a security check with a blanket ignore.

Use the [release process](docs/releases.md) and update the relevant unreleased
notes. New runtime dependencies need a stated purpose, compatibility/license review
and maintainer approval. Generated models, SSE and additional source tracks retain
their separate roadmap acceptance gates. See the [ecosystem review](docs/competitive-landscape.md)
before claiming novelty or replacing an existing component.

## Security checks

On Linux x86_64 (the CI profile):

```bash
python3 scripts/dev.py security install
python3 scripts/dev.py security check all
```

The helper pins upstream versions plus archive and executable SHA-256 values.
It runs cargo-audit 0.23.0, cargo-deny 0.21.0, Gitleaks 8.30.1 and actionlint 1.7.12.
On other platforms install the same upstream tool versions for your host and run
`cargo audit --deny warnings`, `cargo deny check licenses bans sources`,
`gitleaks git --redact --no-banner --ignore-gitleaks-allow --log-opts=--all`, and
`actionlint`. Also scan an export of Git-eligible files for uncommitted changes;
do not scan/upload ignored private credentials. History scanning needs full Git
history (`git fetch --unshallow` when working from a shallow checkout).

Security CI runs on PRs/pushes, manually, and weekly to catch newly published
advisories. It uses read-only permissions and no private service credentials.
Default development passwords are not production secrets; do not introduce broad
scanner allowlists to make a check green. The license policy checks third-party
packages, including development dependencies. Non-publishable workspace packages
remain excluded only from the license check while project licensing is finalized.
