# Aggregate models

Available in `0.4.0-alpha.1`. Install the prerelease explicitly; the stable
0.3.0 CLI does not support this extension. Tag `v0.4.0-alpha.1` and `main`
are also available for source builds.

```bash
cargo install cantongrid-cli --version 0.4.0-alpha.1 --locked
```

A model may declare `aggregation` with `group_by` (one to four distinct projected
field names) and `measures` (one to eight named `count`/`sum` operations). Count
counts active matching contracts; sum names a required Int64 or Numeric field.
Optional grouping/sum fields and name collisions are rejected. Other models keep
their existing declaration fingerprints and APIs.

```json
{
  "aggregation": {
    "group_by": ["status"],
    "measures": {
      "markets": {"op": "count"},
      "total": {"op": "sum", "field": "amount"}
    }
  }
}
```

The engine maintains native projection rows as contributions in the same
transaction as canonical state and checkpoints. Queries calculate grouped results
from those current rows in PostgreSQL. This first slice does not store a second
mutable totals cache. Creates contribute once; archives retract; replay leaves
totals unchanged. Atomic/staged model binding selects the same committed revision
for row and aggregate reads.

Aggregate access first requires every configured source party, independently of
stored data. A valid request from a narrower caller receives denial (REST 403), including when
a group happens to contain only its own inputs. This prevents hidden inputs from
turning group disappearance into a membership oracle. Single-party aggregates use
single-party source views; caller-visible partial totals are not this contract.

Visibility is also the AND of all contributing contracts' witness clauses, with OR
within each clause. For example, inputs visible only to A and only to B produce a
group visible only to AB. A shared input may satisfy its clause through either
party. Hidden contributors are never removed to calculate a caller-specific
partial total. Group counts, pagination and output encoding occur after this
whole-group check. Empty groups disappear. No witness evidence is returned.

Count is an exact string. Int64 sums widen to Numeric(38,0); Numeric sums retain
scale with at most 38 digits. Overflow fails the visible query without rounding.
Optional/null arithmetic is unsupported. Group keys encode ordered typed values;
byte ordering and signed cursors bind pagination to the source, caller, grants,
active definition and aggregate query domain. Each page uses one repeatable-read
snapshot; live pagination does not freeze all pages at a historical watermark.

Generated aggregate GraphQL/TypeScript and a dedicated REST query expose only the
active declaration. Initial requests support bounded limit/cursor pagination;
input filters, aggregate filters, custom sorting, joins and global empty-key
aggregates are deferred. Existing row queries remain available.

Acceptance: Outcomia market counts by status and an unrelated exact-sum model;
full-scope access, narrow-scope denial invariant under membership changes, and shared visibility; hidden-group counts/overflow; create/archive/replay;
failed-batch rollback; binding/rebuild changes; cursor isolation/revocation;
strict declarations and source/caller checks. Label owned-PostgreSQL and LocalNet
evidence separately. No new runtime dependency is planned.

Any model GraphQL resolver error clears the complete operation data, including
errors in mixed row/aggregate aliases.

## Run the examples

The complete declarations are [Outcomia counts](../../examples/models/aggregate-outcomia.json)
and [Visibility count/sums](../../examples/models/aggregate-visibility.json).
Their selectors and contribution expressions use the ordinary model format.

```bash
cargo build -p cantongrid-cli --locked
export CANTONGRID_BIN="$PWD/target/debug/cantongrid"
"$CANTONGRID_BIN" models validate examples/models/aggregate-outcomia.json
"$CANTONGRID_BIN" models plan examples/models/aggregate-visibility.json
"$CANTONGRID_BIN" models typescript examples/models/aggregate-visibility.json
"$CANTONGRID_BIN" models graphql examples/models/aggregate-outcomia.json
```

Register, install and bind an aggregate declaration with the existing
[model lifecycle](models.md). Adding aggregation changes the definition fingerprint:
use a new revision rather than changing an installed revision. Contribution preview
still evaluates one contract; aggregate results require an active source binding.
Upgrade every worker/API handling that binding before activating the new declaration;
0.3.0 rejects it. Migrations 001–006 remain unchanged.

## Query

`GET /v1/models` and model description include `aggregateQueryField` and
`aggregateFields` only for aggregate-enabled bindings. GraphQL discovery exposes
these as nullable fields:

```graphql
query AggregateCatalog {
  models {
    namespace
    name
    revision
    aggregateQueryField
    aggregateFields {
      name
      kind
      scale
    }
  }
}
```

POST `/v1/models/{namespace}/{name}/aggregate` with `{"limit":20,"cursor":null}`.
Each item has an opaque `groupKey` string and typed `values`. `visibleTotal`
counts groups. The generated GraphQL field accepts `namespace`, `limit`, `cursor`
and returns the same page; select `items { groupKey values { status markets } }`
for the Outcomia example. REST/GraphQL cursors interoperate for the same aggregate.
Row cursors, different callers, revoked grants and old revisions cannot reuse them.

No caller-provided input filters/grouping/sorting are accepted. Bad inputs/cursors
return REST 400; insufficient aggregate scope returns 403; caller authentication
and source failures retain the existing 401/503 behavior. Always check GraphQL
`errors`; any resolver failure clears the operation's complete `data`.

## LocalNet check

After completing the normal [PoC](poc.md) with this source checkout:

```bash
python3 examples/poc/demo.py aggregate-check
```

The runner reads a bounded authorized Ledger snapshot, temporarily installs/binds
revision 4, compares exact totals and one-group pages in REST/GraphQL, checks narrow
scope denial and row-cursor isolation, then restores revision 1 even after an
ambiguous activation failure. It makes no ledger writes. Private results stay in
`.local/poc/aggregate-report.json`; [public evidence](../../examples/poc/aggregate-evidence.json)
contains counts/checks and no party or contract IDs.
