# Operations acceptance

This helper uses an already prepared [LocalNet PoC](../../docs/guide/poc.md).
It changes only a new isolated stream in the owned destination: ingestion,
subscription progress and outbox retention. It does not upload DARs, allocate
parties, submit choices or change Ledger rights.

Start the existing LocalNet, PQS AB exporter and owned destination, then:

```bash
cargo build -p cantongrid-cli --locked
python3 examples/operations/localnet.py --allow-owned-writes
```

The PoC must already have five or more active example contracts. Do not run its
ledger-writing setup again merely to run this helper. Source inventory is read
through the existing restricted PQS role; product ingestion and HTTP APIs use Rust.

Checks cover full source-inventory equality at the snapshot offset, temporal
paging, API restart/replay, durable ACK, explicit visible retention loss and
preservation of canonical history. Recovery uses a quiescent controlled fixture;
it does not prove an atomic snapshot/subscription handoff during concurrent trading.

Evidence and recovery state are written under an owner-only `.local/operations`
directory. They contain private runtime configuration; never upload that directory
or database dumps. Only the sanitized `evidence.json` is suitable for review.

## Configuration and recovery

Use [remote source setup](../../docs/guide/remote-source.md) with
`runtime.testnet.json` and `grants.example.json`. These contain placeholders, no
working remote endpoint. `ops check-config --profile testnet` is offline;
`ops preflight --profile testnet` needs authorized source access. A successful
preflight does not independently attest the actual Canton network or the
Ledger/PQS participant pairing.

The [recovery helper](../../docs/guide/recovery.md) verifies an owned database
backup in a separate empty restricted-owner database. It compares native
PostgreSQL rows and sequences, preserving exact numeric values. CI runs this
against the owned fixture. Private dumps are never uploaded as artifacts.

## Database benchmark

With the owned destination running and its connection in the named environment:

```bash
CANTONGRID_DESTINATION_URL='host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid' \
  cargo test -p cantongrid-storage operations_benchmark --locked --offline \
  -- --ignored --nocapture
```

This creates and cleans up its own synthetic stream. Four batches contain 256
contracts visible to one party. On one reused loopback connection it times the
first 25-row temporal page and an unacknowledged 25-event delivery replay, with
five warmups and 50 measured iterations. Percentiles use nearest rank. Each call
includes SQL and JSON decoding; HTTP, source authorization network calls, concurrent
clients and production-sized history are excluded. `BENCHMARK` is a one-line JSON
report suitable for collecting with environment/build details. Do not interpret
these debug-build timings as a throughput guarantee.
