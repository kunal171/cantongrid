# Reconciliation

Prerelease 0.5 checks exact per-contract invariants (for example, collateral ≥
liability) against indexed contracts.
- **0.5.0-alpha.1:** offline policies, schema preflight and storage-level indexed
  checks.
- **0.5.0-alpha.2:** authenticated REST and GraphQL.
- **What it doesn't do:** infer economic correctness or check totals across
  contracts.

```bash
cargo install cantongrid-cli --version 0.5.0-alpha.2 --locked
cantongrid reconcile validate  examples/reconciliation/backing-policy.json
cantongrid reconcile evaluate  examples/reconciliation/backing-policy.json examples/reconciliation/backing-fixture.json
cantongrid reconcile preflight examples/reconciliation/backing-policy.json \
  examples/reconciliation/backing-model.json example-org synthetic_backing
```

The examples are synthetic collateral backing and supply conservation, not
deployed scenarios.

## Policies

A strict version-1 document has one `policy` with `name`, `revision`, `fields`
and `checks`.
- **Fields:** they reuse [model expressions](models.md) and must produce required
  Int64 or Numeric values.
- **Checks:** they compare two fields:

```json
{ "name": "collateral_backing", "left": "collateral", "operator": "gte", "right": "liability" }
```

**Rules.**
- **Operators:** `eq`, `ne`, `lt`, `lte`, `gt` and `gte`.
- **Types:** both sides must be the same type and scale.
- **Not supported:** mixing Int64 and Numeric, defaults, floats, approximate
  equality and SQL.
- **Units:** matching scales don't prove that the assets or units match; that's
  your assumption.

**Limits.**
- **Size:** at most 64 KiB per policy or fixture.
- **Shape:** 1–32 fields and 1–16 checks.
- **Expressions:** depth 16, with 256 nodes in total.
- **Numbers:** scale 0–37, with at most 38 digits.

**Fingerprint.**
- **Coverage:** it covers the revision and the full definition, but not formatting
  or order.
- **Changes:** change a policy by bumping its revision.

## Results

**Offline (`evaluate`).**
- **Per check:** each check reports `satisfied`, `violated` or `unavailable`.
- **Bad input:** a missing value, wrong type or overflow makes **every** check
  `unavailable`. Inputs never default to zero.
- **Report header:** every report is `status: unverified` with
  `coverage: supplied_fixture_only`.
- **Exit code:** a successful exit means a diagnostic was produced, not that the
  checks passed.

**Preflight.**
- **Fields:** every policy field must name exactly one projected model field of
  the same type and scale (use `free`, not `private_balances.free`).
- **Passing result:** `status: schema_compatible` with `sourceAccess: not_performed`.
  It's an offline schema check only.
- **Fingerprint:** `preflight.fingerprint` ties the policy, the model and the
  namespace together.

**Indexed (`Store::read_reconciliation`, and the API).**
- **Access:** requires **every configured source party**, for both the source and
  the caller. Narrower callers are denied.
- **One snapshot:** a single repeatable-read transaction covers the completed
  prefix, the active model, the canonical inputs and the projection rows, so a
  promotion can't mix revisions.
- **What it detects:** re-evaluating the canonical inputs catches missing,
  different or extra projection rows.
- **Limits:** 128 active contracts before model filtering, 8 MiB of input, a 5 s
  statement timeout, and the historical-state budget.
- **When it's unavailable:** over budget, empty or fully filtered, partial or
  stale, or a changed binding. There are no partial counts in that case.

```json
{ "status": "evaluated", "comparison": "satisfied", "coverage": "retained_indexed_prefix",
  "completeWithinConfiguredScope": true, "contracts": "2",
  "checks": [{ "name": "collateral_backing", "satisfied": "2", "violated": "0", "unavailable": "0" }] }
```

- **Comparison:** `comparison` is unavailable if any calculation failed.
  Otherwise any violation wins.
- **What's left out:** reports never include offsets, contract IDs, payloads or
  values, so keep configured names free of secrets.

## API (0.5.0-alpha.2)

Add `"reconciliation_file": "reconciliation/policies.json"` to `runtime.json`.
- **Bindings:** the manifest holds 1–16 named bindings, each pinning `namespace`,
  `model`, `model_revision`, `model_fingerprint`, `policy_file` and
  `policy_fingerprint`.
- **Where pins come from:** preflight. Update them when you promote a model or
  policy.
- **Bad or stale manifest:** an invalid manifest or a stale pin makes
  reconciliation unavailable.

| Request | Purpose |
| --- | --- |
| `GET /v1/reconciliation` | The configured identities |
| `POST /v1/reconciliation/{name}/query` with `{}` | Evaluate one binding |
| `POST /reconciliation/graphql` | `reconciliationPolicies`, `reconciliation(name)` |

**Request rules.**
- **Inputs:** requests can't supply formulas, paths, models or party claims.
- **Order of checks:** authorization comes before any lookup.
- **Limits:** responses are `no-store` and at most 2 MiB. GraphQL allows at most
  2 roots.

**Status codes.**
- **401:** an invalid caller.
- **403:** an incomplete scope.
- **400:** bad input.
- **404:** an unknown name (only after the scope check).
- **503:** something is unavailable.

An arithmetic `unavailable` is still an HTTP 200 diagnostic.

**LocalNet check:**
`python3 examples/operations/localnet.py --allow-owned-writes --reconciliation`.
- **Method:** it compares the Ledger and PQS inventories at one boundary, checks
  both APIs, a deliberate violation, a narrow-scope denial, a revocation and a
  restart.
- **Writes:** it makes no ledger writes.
- **Result:** 7 retained PoC contracts passed. These were unfunded term checks,
  not collateral.
