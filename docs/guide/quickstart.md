# Quickstart

Requirements: Git, Rust/Cargo 1.100.0; Python 3.11+ and Docker/Compose for the
optional PostgreSQL step. Run commands from the repository root.

## Install from crates.io

```bash
cargo install cantongrid-cli --version 0.4.0 --locked
cantongrid --version
cantongrid --help
```

The binary is named `cantongrid`; Cargo installs it under `~/.cargo/bin` by default.
The examples below use a repository checkout for configuration and fixture files.

## Build from source

```bash
git clone https://github.com/kunal171/cantongrid.git
cd cantongrid
df -h /
cargo +1.100.0 build -p cantongrid-cli --locked
export PATH="$PWD/target/debug:$PATH"
cantongrid --version
cantongrid --help
```

With rustup installed, `rust-toolchain.toml` selects Rust 1.100.0, rustfmt and Clippy.
If `CARGO_TARGET_DIR` is set, use its `debug` directory in PATH. Reuse one build
directory; after fetching dependencies, Cargo checks can use `--offline`.

## Run without Canton or Docker

```bash
cantongrid validate examples/fixtures/visibility/cantongrid.json
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party B
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A --party B
cantongrid fixture run examples/fixtures/visibility/cantongrid.json
```

| Simulated grants | `visible_active` | `visible_history` |
| --- | --- | --- |
| A | 2 | 4 |
| B | 2 | 2 |
| A + B | 3 | 5 |
| None | 0 | 0 |

Each run starts in memory and reports two applied batches, then zero on its
internal replay. `--party` simulates grants; it does not authenticate real users.

To create a new local fixture project:

```bash
cantongrid init ./my-cantongrid-demo
cantongrid validate ./my-cantongrid-demo/cantongrid.json
cantongrid fixture run ./my-cantongrid-demo/cantongrid.json --party A
```

`init` requires a new directory and writes `cantongrid.json` plus `events.json`.
It scaffolds a fixture, not a live source. Event paths are relative to the project
file. Keep the local demo uncommitted unless contributing it intentionally.

## Persist in PostgreSQL

Docker Compose must support `up --wait`. This creates a separate disposable
PostgreSQL destination on loopback 5442, with a public development password:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture index examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture status examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture query examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL --party A
```

Repeat `fixture index`: zero new batches should apply. `status` shows operator
counts; `query` applies simulated caller grants to the stored journal. Querying
already indexed data does not need the original events file.

Only `storage init` applies destination migrations. Never target upstream PQS.
`python3 scripts/dev.py storage stop` stops containers but retains the volume.
The fixture journal has [bounded replay limits](../architecture.md#destination-and-recovery).

Next: [LocalNet ingestion](localnet-source.md), [API](api.md) or [models](models.md).
