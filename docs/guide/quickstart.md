# Install and run your first example

Requirements: Git and Rust/Cargo **1.100.0**. No Docker, Daml SDK, source token or
database is required for this page. Python 3.11+ is used by later development helpers.

## Build the CLI

```bash
git clone https://github.com/kunal171/cantongrid.git
cd cantongrid
rustup toolchain install 1.100.0 --profile minimal --component rustfmt --component clippy
df -h /
cargo +1.100.0 build -p cantongrid-cli --locked
export PATH="$PWD/target/debug:$PATH"
cantongrid --version
cantongrid --help
```

If you set `CARGO_TARGET_DIR`, add that directory's `debug` folder to PATH instead.
Reuse one build directory. After dependencies have been fetched, Cargo also accepts
`--offline`. These instructions build from source; no package-manager installation
or prebuilt binary distribution is assumed.

## Run the Visibility fixture

```bash
cantongrid validate examples/fixtures/visibility/cantongrid.json
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party B
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A --party B
cantongrid fixture run examples/fixtures/visibility/cantongrid.json
```

The JSON output identifies `mode: fixture`, `storage: memory` and
`authentication: simulated_fixture_grants`. The result includes lifecycle/replay
and caller-filtered counts. Omitting parties yields zero visible records. Each run
starts from scratch and internally replays the delivery to check idempotency.

Expected caller-visible counts for the checked-in fixture:

| Simulated grants | `visible_active` | `visible_history` |
| --- | --- | --- |
| A | 2 | 4 |
| B | 2 | 2 |
| A + B | 3 | 5 |
| None | 0 | 0 |

Each run reports `applied_batches: 2` and `replay_applied_batches: 0`. History
includes archived contracts, which is why its counts can exceed active state.

`--party` is a simulation flag. Real API users cannot select their permissions
with a query parameter or this flag.

## Scaffold a project

Choose a directory name that does not exist:

```bash
cantongrid init ./my-cantongrid-demo
cantongrid validate ./my-cantongrid-demo/cantongrid.json
cantongrid fixture run ./my-cantongrid-demo/cantongrid.json --party A
```

`init` creates `cantongrid.json` and `events.json` for the synthetic Visibility
fixture. It refuses an existing directory. The fixture path is relative to the
configuration file, so the project can be moved together with its event file.
Keep this local demo uncommitted unless you intend to contribute a fixture.

Today `init` scaffolds a fixture, not a live Ledger/PQS connection. Live runtime
configuration has a separate schema and workflow.

Next: [persist the same fixture](postgresql.md), or try an
[offline declarative model](models.md).
