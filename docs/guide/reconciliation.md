# Offline reconciliation policies

Available in the **unreleased 0.5.0-alpha.1 source checkout**. This first slice
validates versioned exact-value policies and compares supplied local fixtures.
It does not inspect indexed models, authenticate source data or certify balances.
No dependency, database migration or API route is added.

```bash
cantongrid reconcile validate examples/reconciliation/backing-policy.json
cantongrid reconcile evaluate examples/reconciliation/backing-policy.json examples/reconciliation/backing-fixture.json
```

[Examples](../../examples/reconciliation/README.md) include synthetic collateral
backing and supply conservation. They are not deployed Daml/Outcomia scenarios.

## Define a policy

A strict version-1 document contains one `policy` with `name`, positive `revision`,
`fields` and `checks`. Fields reuse the existing [model expressions](models.md):
object-field paths, typed literals, checked add/subtract and supported multiply.
Only required Int64 or Numeric outputs are allowed. Check operands name fields:

```json
{
  "name": "collateral_backing",
  "left": "collateral",
  "operator": "gte",
  "right": "liability"
}
```

Operators are `eq`, `ne`, `lt`, `lte`, `gt` and `gte`. Both operands must have
exactly the same scalar type and Numeric scale. Int64/Numeric mixed comparison,
optional defaults, floating-point amounts, approximate equality and arbitrary SQL
are unsupported. Asset/unit agreement is a developer assumption in this offline
slice, not an inference made from matching Numeric scales.

| Bound | Limit |
| --- | --- |
| Policy / fixture bytes | 64 KiB each |
| Named fields | 1–32 |
| Checks | 1–16, unique names |
| Expression depth / total nodes | 16 / 256 across the whole policy |
| Numeric scale / precision | 0–37 / at most 38 digits |

Names follow the existing model identifier rules. Unknown schema fields,
duplicate JSON keys, nonpositive revisions and unsupported operators/types fail
validation. A policy fingerprint includes its revision and complete definition;
formatting, field-map order and check order do not affect it. The compiler does
not register identities persistently or enforce cross-file revision immutability.
Use a new revision when a policy changes.

## Interpret the result

`validate` confirms only the policy definition and prints its identity/check count.
`evaluate` prints `report.checks`, with a `fixtureComparison` for each named check:
`satisfied`, `violated` or `unavailable`, plus a finite `errorCode` where needed.
Missing/null values, wrong types/scales and arithmetic overflow make **all** checks
unavailable rather than producing partial success. Required inputs never default
to zero. Values above 2^53 and last-decimal-digit differences remain exact.

Every report has:

```json
{
  "status": "unverified",
  "coverage": "supplied_fixture_only",
  "sourceScope": null,
  "sourceOffset": null,
  "completeWithinConfiguredScope": false
}
```

CLI success means a diagnostic was produced; it does not mean the comparisons
were satisfied. Malformed policies, unsupported command shapes and local file
read/encoding/size errors exit unsuccessfully. Commands never load runtime source
configuration or obtain database/ledger permissions. Fixtures cannot override
report evidence by supplying their own coverage or authorization claims.

Reports include configured names, revision, fingerprint and finite codes; they
omit payload values, paths, source parties, contract IDs and raw parser errors.
Keep secrets out of configured policy/check names. The shared evaluator preserves
existing model fingerprints and projection behavior.

## Live checks are a separate step

Before live reconciliation, the [design gate](../design/reconciliation.md#live-track-gate)
requires coherent source/model-revision evidence, complete configured party scope
before any reads/counts, exact asset/unit selection, current authorization and
revocation checks, and unavailable/incomplete outcomes for source gaps. Global
source positions must stay operator-private or use opaque evidence handles.
There is no live REST/GraphQL, persisted invariant-result store or financial
all-clear endpoint in this slice. Fixture arithmetic cannot stand in for that work.
