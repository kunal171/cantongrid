# Offline reconciliation fixtures

Requires the unreleased 0.5.0-alpha.1 source checkout:

```bash
cargo build -p cantongrid-cli --locked
export PATH="$PWD/target/debug:$PATH"
cantongrid reconcile validate examples/reconciliation/backing-policy.json
cantongrid reconcile evaluate examples/reconciliation/backing-policy.json examples/reconciliation/backing-fixture.json
cantongrid reconcile evaluate examples/reconciliation/supply-policy.json examples/reconciliation/supply-fixture.json
```

These are synthetic inputs, unrelated to deployed contract schemas or actual funds:

| Policy | Configured equation | Exact fixture |
| --- | --- | --- |
| [Collateral backing](backing-policy.json) | `free + locked >= payout × contracts` | `0.1 + 0.2 >= 0.1 × 3` |
| [Supply conservation](supply-policy.json) | `issued = circulating + redeemed` | `10^19 = 9×10^18 + 10^18` |

All amounts are Numeric(10) strings; contract count is an Int64 string. Inputs must
use one consistent asset/unit by developer convention. This slice does not verify
assets, units, source rights or ledger coverage. The backing example does not
assert that Outcomia has deployed funded escrow contracts.

Individual `fixtureComparison` values are `satisfied`, `violated` or `unavailable`.
The report always remains `status: unverified`, `coverage: supplied_fixture_only`,
with null source scope/offset and `completeWithinConfiguredScope: false`.
A missing input or overflow cannot count as a satisfied check. A successful CLI
exit means a diagnostic was produced, including for violated/unavailable fixtures.
It is not a financial all-clear or a deployment/CI approval signal.

Policy/check names and fingerprints are intentional report metadata. Payloads,
amounts, field paths and raw errors are not echoed. Keep sensitive data out of
policy/check names. Fixtures cannot add verified evidence by declaring `complete`
or `sourceScope` fields. Commands read only the two supplied local files; no
runtime config, credentials, database, HTTP service or ledger transaction is used.

The matching copies under crate `assets/reconciliation` keep registry-package
tests self-contained. `scripts/dev.py release check` checks them against these
canonical example files. See the [policy guide](../../docs/guide/reconciliation.md).
