# Reconciliation

Available in **0.5.0-alpha.1**: offline exact
policies, model-schema preflight and storage-only indexed per-contract checks.
The CLI commands read explicit local files; the Rust storage interface below
separately checks authorized indexed inputs. Neither infers economic correctness.
No dependency, database migration or public API route is added.

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

## Preflight a projected model schema

```bash
cantongrid reconcile preflight \
  examples/reconciliation/backing-policy.json \
  examples/reconciliation/backing-model.json example-org synthetic_backing
```

Select a model by name from the bounded catalog. Every policy field path must
name exactly one projected field with the same required scalar type and Numeric
scale. A model may read `private_balances.free` from a raw payload while exposing
`free`; the policy must reference `free`. Nested/raw paths, missing fields,
optional/type/scale mismatches and constant-only checks fail. Constants remain
valid thresholds when the check also depends on a model field. All policy
expressions are validated, including unused ones.

The diagnostic's `preflight.fingerprint` binds the full policy and model
identities plus namespace. Changing the model filter, template, expression,
revision or namespace changes it. Catalog formatting/order does not. The
namespace uses the registry's syntax: 1–64 ASCII characters, an initial letter,
then letters, digits, underscores or hyphens.

`status: schema_compatible`, `sourceAccess: not_performed` and
`activeBindingChecked: false` mean only offline schema compatibility. No model
is installed or bound; no database state, source coverage or permissions are
checked. Aggregate declarations still describe per-contract inputs here; no
sum/group query is performed. This fingerprint is not an authorization token.
Output includes configured model/namespace/policy identities; keep their names
nonsecret. Errors omit raw catalog paths and definitions.

## Indexed per-contract checks (Rust storage API)

The same prerelease also implements `Store::read_reconciliation`:

```rust
let report = store.read_reconciliation(
    &plan, &caller, &source_lease, &active_binding, &policy,
)?;
```

This is a trusted backend interface, with no new CLI network command or HTTP
route. Authenticate `CallerGrants` independently and obtain `SourceLease` from the
existing trusted source checker. Select an exact `BoundModel` through current
binding discovery; the read rechecks its installed definition and active revision.
A policy/preflight document cannot provide any of these permissions.

Reads require **every configured source party** for both source authority and the
caller, before any data/count lookup. Narrow callers are denied regardless of
input membership. One read-only repeatable-read transaction covers the completed
schema041 prefix, retained journal coverage, fresh ingestion observation, active
model, canonical inputs and native projection rows. A concurrent ingestion or
model promotion cannot mix revisions within a report. Source leases are rechecked
before and after commit; an HTTP adapter must separately reauthenticate current
caller/source grants before encoding a future public response.

Limits: 128 active canonical contracts for the selected template **before model
filtering**, 8 MiB of canonical input plus expected projection encoding, existing
model/policy expression limits and the destination's five-second statement
timeout. Retained coverage uses the existing bounded temporal verifier (4,096
batches, 8,192 events and 64 selection upgrades). Excess work, empty/all-filtered
selections, partial snapshots/intervals, stale observations, missing history,
changed bindings or inconsistent projections return
`StoreError::ReconciliationUnavailable` without partial counts. Source/caller
scope failures return `StoreError::Authorization`.

Canonical reevaluation and native PostgreSQL record equality detect missing,
different or extra contributions, including filtered/archived rows. No repair or
freshness update is performed. Int64/Numeric values never pass through floats.

A successful report contains configured policy/model identities,
`definitionFingerprint`, exact string counts and these separate fields:

```json
{
  "status": "evaluated",
  "comparison": "satisfied",
  "coverage": "retained_indexed_prefix",
  "completeWithinConfiguredScope": true,
  "contracts": "2",
  "checks": [
    {"name": "collateral_backing", "satisfied": "2", "violated": "0", "unavailable": "0"}
  ]
}
```

`comparison` is unavailable if any required calculation fails; otherwise any
violation dominates satisfaction. For example, one satisfied input and one
arithmetic overflow yield unavailable, even though input coverage is complete.
Coverage is only this exact model's selected per-contract inputs at a complete
retained indexed prefix, not the latest ledger head or history before the initial
snapshot. This does not check cross-contract totals, asset agreement or economic
correctness. The definition fingerprint identifies schemas/formulas, not a source
snapshot, replay cursor or signed receipt. Reports omit global offsets, contract
IDs, witnesses, payloads and financial values; configured names must be nonsecret.

Database controls use synthetic source authority with real owned PostgreSQL:

```bash
python3 scripts/dev.py storage up
# Configure CANTONGRID_DESTINATION_URL for this owned test database; see quickstart.
cargo test -p cantongrid-storage model_projection::reconciliation --locked --offline -- --ignored --test-threads=1
python3 scripts/dev.py storage stop
```

These checks are separate from LocalNet/remote evidence. There is no new
REST/GraphQL endpoint, persisted invariant-result registry or financial all-clear.
Cross-model/group reconciliation and public API delivery remain future slices.

## Authenticated API (0.5.0-alpha.2 source)

Build this branch with `cargo build -p cantongrid-cli --locked`. Add an optional
setting to the existing runtime configuration:

```json
{"reconciliation_file": "reconciliation/policies.json"}
```

The path resolves relative to `runtime.json`. The
[example manifest](../../examples/reconciliation/policies.json) contains 1–16
uniquely named bindings, each with `namespace`, `model`, `model_revision`,
`model_fingerprint`, `policy_file` and `policy_fingerprint`. Policy paths resolve
relative to the manifest. Both files have a 64 KiB bound. Reuse one policy under
different aliases if needed. Obtain the pins from model preflight and deliberately
update them when promoting a model or policy revision. The example's zero package
ID and namespace are synthetic; substitute your installed model's identity.

Run the usual `cantongrid serve runtime.json` with an organization caller bearer
covering every configured source party. No additional SQL, migration or source
grant is required. Omitting the setting disables the catalog; an invalid manifest
or inactive/incompatible pin makes reconciliation unavailable. Other endpoints
and ingestion do not load optional policy contents.

| Transport | Operation |
| --- | --- |
| `GET /v1/reconciliation` | Sanitized configured identities |
| `POST /v1/reconciliation/{name}/query` | Evaluate the named binding; body must be exactly `{}` |
| `POST /reconciliation/graphql` | Typed `reconciliationPolicies` and `reconciliation(name: String!)` |

```bash
curl -H "Authorization: Bearer $CANTONGRID_API_TOKEN" \
  -H 'Content-Type: application/json' -d '{}' \
  http://127.0.0.1:8090/v1/reconciliation/backing/query
```

Use your configured API port. The [complete GraphQL query](../../examples/reconciliation/query.graphql)
takes `{"name":"backing"}` variables and selects the same report as REST:

```graphql
query Reconcile($name: String!) {
  reconciliation(name: $name) {
    comparison
    contracts
    checks {
      name
      satisfied
      violated
      unavailable
    }
  }
}
```

Requests cannot supply formulas, paths, model selections, party claims or input
filters. Authorization precedes catalog/policy lookup, including for unknown
names. Admission and a final refresh check caller grants, exact source scope,
policy identities, source plan and active model identities. Final serialization
also rechecks local config/caller bindings and the live lease. No-store headers,
existing request/database limits and the 2 MiB response bound apply. GraphQL
allows at most two roots per operation, disables introspection and clears its
entire `data` on any resolver error.

REST uses 401 for invalid/changed caller bindings or a final delivery-guard
failure, 403 for incomplete scope, 400 for unsupported input, 404 for an unknown
alias after full-scope checks, and 503 for unavailable source/configuration/pins
or indexed evidence. GraphQL validation/resolver errors use its `errors` array;
always inspect it. An arithmetic `comparison: unavailable` is a successful HTTP
200 diagnostic with complete inputs, using the storage semantics above.
