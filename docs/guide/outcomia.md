# Outcomia prediction-market example

Outcomia demonstrates using the same engine for a prediction-market application.
There is no Outcomia-specific ingestion or API branch in Rust. Keep these three
examples distinct: a synthetic lifecycle fixture, an offline model projection,
and the separately provisioned LocalNet integration.

## Synthetic lifecycle fixture

```bash
cantongrid fixture run examples/fixtures/outcomia/cantongrid.json --party A
```

This requires only the CLI. It exercises the fixture's canonical lifecycle and
simulated visibility, not funded assets or real ledger transactions. Use the
[PostgreSQL workflow](postgresql.md) with the same Outcomia fixture path to
persist/replay it.
For party A, the current fixture reports one visible active contract, three
visible history events and zero batches applied on the internal replay.

## Preview a market projection

```bash
cantongrid models validate examples/models/outcomia.json
cantongrid models preview examples/models/outcomia.json markets examples/models/outcomia-payload.json
cantongrid models plan examples/models/outcomia.json
```

The model pins the reviewed `Outcomia.Binary:BinaryMarket` package and extracts
market ID, protocol name, question, status, quantity lot and price tick. The
checked-in preview payload is synthetic and deliberately includes boundary values:

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

The derived `priceTickBps` expression multiplies the Numeric price tick by the
Int64 literal `10000`, yielding **`0.0000010000`** at scale 10. Quantity lot stays
the exact maximum signed Int64 string. This calculation is a projection example,
not proof that a token balance, probability or accounting relationship is valid.

`Closed` means trading has stopped in this lifecycle; it does not prove final
resolution or settlement. Custody, orders, positions and resolution have separate
contracts. Do not turn one market row into a user's balance or infer a globally
unique market identity from operator/name alone.

## Existing LocalNet integration

The [recorded acceptance](../phase3-verification.md) verified an actual
Protocol/Draft/Open/Closed lifecycle through PQS, owned PostgreSQL and both API
transports, including paged snapshot publication and restarts. That proof used a
separately authorized local DAR upload and test transactions. It did not create
funded collateral, orders, trades or resolutions.

You can query existing authorized Outcomia contracts with the generic
[runtime setup](localnet-source.md) and [APIs](querying.md), selecting the exact
package/module/entity visible to your source user. The offline `markets` model
does not yet create a `markets` GraphQL field.

To reproduce the write-enabled demonstration, first obtain authorization for the
disposable LocalNet and the existing Outcomia DAR. The command and side effects
are documented in [the runtime demonstration](../live-runtime.md). It is not part
of this guide's default read-only ingestion path. CantonGrid does not require a
sibling Outcomia checkout for its fixture/model examples.
