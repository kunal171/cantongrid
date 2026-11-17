# Credential-free ingestion examples

Both examples use synthetic parties, contract IDs, event IDs, timestamps and
positions. They are hand-authored complete lifecycle inputs, not exported ledger
history. They never contact Canton, create users or submit transactions.

- `visibility`: the unrelated [Visibility:Record](../visibility/daml/Visibility.daml)
  fixture, with A-only, B-only and joint witnesses, an archive and a successor.
  Numeric 10 and Int64-max strings exercise precision preservation.
- `outcomia`: `Outcomia.Binary:Protocol`, using the source fields reviewed in
  [Outcomia compatibility](../../docs/outcomia-compatibility.md): operator,
  protocolName, admins, nextMarketId, takerFeeBps and feeRevision. A consuming
  revision archives the old contract and creates a successor. This is a
  prediction-market application's protocol configuration example, not a full
  market/trading or ledger execution demonstration.

Package IDs refer to the recorded fixture build and validated Outcomia DAR.
These files are explicit generic typed payloads; they do not prove full Daml
metadata validation or that a source's current package/source code is identical.
No domain-specific behavior is implemented in the Rust engine.

Expected counts after replay:

| Example | Fixture caller | Active | History |
| --- | --- | --- | --- |
| visibility | A | 2 | 4 |
| visibility | B | 2 | 2 |
| visibility | A + B | 3 | 5 |
| visibility | no parties | 0 | 0 |
| outcomia | A or B | 1 | 3 |

The opaque `fixture-z` position precedes `fixture-a` by declared delivery order.
Neither lexical ordering nor arithmetic on source positions is meaningful.
