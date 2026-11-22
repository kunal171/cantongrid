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
