# Quickstart

You need Git and Rust 1.100.0. The PostgreSQL step also needs Python 3.11+ and
Docker Compose. Run every command from the repository root.

## Install

```bash
cargo install cantongrid-cli --version 0.4.0 --locked
cantongrid --version
```

Or build from source (`rust-toolchain.toml` pins the toolchain):

```bash
git clone https://github.com/kunal171/cantongrid.git && cd cantongrid
cargo build -p cantongrid-cli --locked
export PATH="$PWD/target/debug:$PATH"
```

## Fixtures: no Canton, no Docker

```bash
cantongrid validate examples/fixtures/visibility/cantongrid.json
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A
cantongrid fixture run examples/fixtures/visibility/cantongrid.json --party A --party B
```

| Grants | `visible_active` | `visible_history` |
| --- | --- | --- |
| A | 2 | 4 |
| B | 2 | 2 |
| A + B | 3 | 5 |
| None | 0 | 0 |

- **Batches:** each run applies two batches in memory, then zero when it replays
  them.
- **`--party`:** it simulates grants; it doesn't authenticate anyone.
- **New project:** `cantongrid init ./my-demo` creates a new fixture project with
  `cantongrid.json` and `events.json`. Event paths are relative to the project file.

## PostgreSQL

This starts a disposable destination on `127.0.0.1:5442`, with a public dev
password:

```bash
python3 scripts/dev.py storage up
export CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'
cantongrid storage init --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture index  examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture status examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL
cantongrid fixture query  examples/fixtures/visibility/cantongrid.json --database-env CANTONGRID_DESTINATION_URL --party A
```

- **Idempotent:** running `fixture index` again applies zero batches.
- **Migrations:** only `storage init` runs migrations. Never point it at upstream
  PQS.
- **Stopping:** `scripts/dev.py storage stop` stops the containers and keeps the
  volume.

Next: [LocalNet](localnet.md), [API](api.md) or [models](models.md).
