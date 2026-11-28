# Outcomia example

Outcomia (prediction markets on Canton) runs on the same engine as the Visibility
example. All of its domain logic lives in application definitions; there are no
Outcomia branches in the Rust code.

## Offline

```bash
cantongrid fixture run examples/fixtures/outcomia/cantongrid.json --party A
cantongrid models validate examples/models/outcomia.json
cantongrid models preview  examples/models/outcomia.json markets examples/models/outcomia-payload.json
cantongrid models plan     examples/models/outcomia.json
```

- **Fixture:** one active contract, three history events, and zero new batches
  on replay.
- **Model:** pins `Outcomia.Binary:BinaryMarket` and projects market ID, protocol,
  question, status, lot and tick.
  - **Example:** `priceTickBps = priceTick × 10000` turns `0.0000000001` into
    `0.0000010000`.
  - **Exact values:** the lot stays an exact Int64 string.
- **Statuses and contracts:** `Closed` stops trading but isn't settlement. Custody,
  orders and positions are separate contracts.

## LocalNet run

With permission to upload the DAR and write to the ledger:

```bash
CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check outcomia-pipeline \
  --dar ../outcomia-canton/contracts/main/.daml/dist/outcomia-markets-0.1.0.dar --allow-ledger-writes
```

**What it does.**
- **Markets:** submits A-only, B-only and shared markets while the worker runs.
- **Edge values:** lots of `2`, `10` and `9007199254740993`, and ticks of
  `0.02` and `0.10`.
- **Per page:** checks the visible count, contract identity and values.
- **Isolation:** compares discovery and cursors across transports, and rejects
  changed callers, filters and revisions.
- **Revocation:** disables and restores caller B.
- **Revision 2:** adds the optional `closeReason`, checks null filtering, then
  rolls back.
- **Final check:** reconciles the canonical active set with what was submitted.

**Notes.**
- **Credentials:** exporter credentials expire after 1 hour. Renew them with
  `scripts/dev.py pqs up --visibility`.
- **Evidence:** `examples/fixtures/outcomia/model-query-evidence.json` records the
  commit, the DAR hash and the environment.
- **Not covered:** funded trading.
