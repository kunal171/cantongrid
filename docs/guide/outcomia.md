# Outcomia example

Outcomia uses the same engine as the unrelated Visibility example. Its domain
logic stays in application definitions, not Rust ingestion or API branches.

## Fixture and model

```bash
cantongrid fixture run examples/fixtures/outcomia/cantongrid.json --party A
cantongrid models validate examples/models/outcomia.json
cantongrid models preview examples/models/outcomia.json markets examples/models/outcomia-payload.json
cantongrid models plan examples/models/outcomia.json
```

The synthetic fixture reports one active contract, three history events and zero
new batches on internal replay. Use the [PostgreSQL workflow](quickstart.md#persist-in-postgresql)
with its project path to persist it.

The model pins `Outcomia.Binary:BinaryMarket` and projects market ID, protocol,
question, status, lot and price tick. Its synthetic preview input is:

```json
{
  "marketId": "7",
  "protocolName": "example-protocol",
  "status": "Closed",
  "terms": {
    "question": "Will the example event happen?",
    "priceTick": "0.0000000001",
    "quantityLot": "9223372036854775807"
  }
}
```

`priceTickBps = priceTick × 10000` yields `0.0000010000` at scale 10. Quantity lot
remains the maximum signed Int64 string. Preview does not install a table or a
`markets` GraphQL root; see [models](models.md).

`Closed` stops trading; it does not establish final resolution or settlement.
Custody, orders and positions have separate contracts. A market row is not a
balance, and operator/name alone is not a globally unique identity.

## LocalNet demonstration

The [recorded alpha.4 run](../verification.md#recorded-localnet-acceptance)
uses the approved existing DAR and real Protocol/Draft/Open/Closed lifecycles.
It covers model queries and private/shared scopes without funded assets, trades
or resolutions.

With permission for local DAR upload and ledger writes, reproduce using:

```bash
CANTONGRID_BIN=target/debug/cantongrid python3 scripts/dev.py check outcomia-pipeline \
  --dar ../outcomia-canton/contracts/main/.daml/dist/outcomia-markets-0.1.0.dar \
  --allow-ledger-writes
```

This requires the existing DAR, owned test parties/writer and configured LocalNet
source/destination. It retains isolated test protocols/market revisions and does
not reset volumes or modify the sibling repository. Use the package-name command
identifier with the exact package-ID preference, as the helper does.

The runner binds revision 1 of the supplied application model and submits real
A-only, B-only and shared markets while the Rust worker runs. Lot sizes `2`, `10`
and `9007199254740993` check numeric ordering and exact Int64 transport. Valid
price ticks `0.02`/`0.10` check exact and derived-value filters. Every one-row
page checks the visible count, actual contract identity and values.

It compares discovery and cursors across transports, rejects changes to caller,
filters, sorting and revision, then disables/restores caller B with increasing
grant epochs. Revision 2 adds the contract's optional `closeReason` field;
null/non-null filtering and nulls-last ordering are verified before rollback to
revision 1. New intermediate create/archive events and the final canonical active
set are reconciled with submission results. Generated TypeScript is saved under
the run's ignored `.local/outcomia-pipeline/<run>/generated/` directory.

Local exporter credentials expire after one hour. Renew them with
`python3 scripts/dev.py pqs up --visibility` before a later run. Source preflight
fails before DAR upload or test submission when the exporter is unhealthy.
The captured [model-query evidence](https://github.com/kunal171/cantongrid/blob/main/examples/fixtures/outcomia/model-query-evidence.json)
records the exact tested commit, DAR hash and environment. Larger staged model
backfills and funded trading projections are not established by this check.

A sibling checkout is not needed for the synthetic examples. To query existing
authorized contracts, use [LocalNet setup](localnet-source.md) and [API](api.md)
with the verified package selection. Artifact hashes are in [verification](../verification.md#outcomia-artifact).
