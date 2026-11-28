# Models

A model picks fields from a contract template and computes derived values with
exact arithmetic.
- **Lifecycle:** validate, register an immutable revision, install its table,
  then bind it to a source. From then on, ingestion keeps the rows current.
- **Generated clients:** models generate TypeScript and GraphQL; see the
  [API](api.md#model-queries).

## Write a definition

```json
{ "version": 1,
  "models": [{
    "name": "quotes", "revision": 1,
    "template": { "package_id": "<64 hex>", "module": "Example", "entity": "Quote" },
    "fields": {
      "price": { "op": "field", "path": ["terms", "price"], "as": { "type": "numeric", "scale": 2 } },
      "doublePrice": { "op": "multiply",
        "left":  { "op": "field", "path": ["terms", "price"], "as": { "type": "numeric", "scale": 2 } },
        "right": { "op": "literal", "value": { "type": "int64", "value": "2" } } },
      "note": { "op": "field", "path": ["note"], "as": { "type": "optional", "item": { "type": "text" } } }
    },
    "filter": { "field": "state", "equals": { "type": "text", "value": "Ready" } } }] }
```

```bash
cantongrid models validate examples/models/visibility.json
cantongrid models preview  examples/models/visibility.json records examples/models/visibility-payload.json
cantongrid models plan     examples/models/visibility.json      # DDL, applied: false
```

**Preview.**
- **Match:** preview returns `matched` plus typed values. With a payload price of
  `"1.2"`, `price` is `1.20` and `doublePrice` is `2.40`.
- **Errors:** `1.234` is rejected at scale 2 rather than rounded. A missing
  required field is an error even if the filter would drop the row.

| Type or op | Rule |
| --- | --- |
| `text`, `party`, `bool` | JSON string, a valid party, a JSON boolean |
| `int64` | Decimal string within signed 64-bit |
| `numeric` | Decimal string, scale 0–37, at most 38 digits |
| `optional` | One scalar layer; missing or null means None |
| `add`, `subtract` | Int64 pairs, or Numeric pairs of the same scale |
| `multiply` | Int64 × Int64, or Numeric × Int64 |
| `literal` | A typed scalar |

**Rules.**
- **Paths:** they go through objects only, never arrays.
- **Expressions:** they read input fields, not other outputs.
- **Overflow:** it fails evaluation.
- **Filter:** the optional `filter` is one equality on a projected field. It
  never widens visibility.
- **Not supported:** Numeric × Numeric, arithmetic on optionals, SQL and plugins.

**Limits.**
- **Strict JSON:** no duplicate or unknown keys, and no floats.
- **Document:** at most 64 KiB, with 1–16 models.
- **Model:** 1–32 fields, 256 expression nodes, depth 16, and path segments of at
  most 16.
- **Data size:** payloads up to 64 KiB, and projections up to 1 MiB.
- **Text:** text containing NUL is rejected.

## Register, install, bind

```bash
cantongrid models register examples/models/visibility.json --namespace demo --database-env CANTONGRID_DESTINATION_URL
cantongrid models install demo records 1 --database-env CANTONGRID_DESTINATION_URL
cantongrid index .local/runtime.json --once          # needs one committed page first
cantongrid models bind .local/runtime.json demo records 1
```

- **Register:** writes the catalog atomically.
  - **Repeating:** returns `unchanged`.
  - **Changes:** changing an existing revision fails, so make a new one.
  - **Namespaces:** they're operator labels of up to 64 ASCII characters, not
    permission scopes.
- **Install:** creates the table.
  - **Storage types:** `numeric(38, scale)`, `bigint`, nullable optionals, and a
    parent contract key.
  - **What it checks:** ownership, grants, column types, indexes, and that there
    are no triggers or rules.
  - **Drift:** schema drift is rejected, not repaired.
  - **Repeating:** safe, including after a lost acknowledgement.
  - **Shared tables:** identical definitions share one physical table.
- **Bind:** checks source rights and readiness, then backfills known active
  contracts and activates the revision in one transaction.
  - **Ingestion:** after binding, a create inserts a matched row and an archive
    removes it.
  - **Rollback:** a projection error, an expired lease or drift rolls back the
    whole page, including history and checkpoint. Events are never skipped.
- **Atomic limits:** 128 active contracts and 8 MiB of work per backfill, and at
  most 16 bindings per stream.
  - **Larger sets:** use a staged rebuild.
  - **Coverage:** backfill covers the known prefix only. It can't recover history
    from before the snapshot.

**Promote or roll back.** Register and install the new revision, then run:

```bash
cantongrid models bind .local/runtime.json demo records 2 --replace 1
cantongrid models bind .local/runtime.json demo records 1 --replace 2   # rollback, rebuilds from the canonical prefix
```

- **No gaps:** promotion takes the ingestion lock, so no event slips between
  backfill and live processing.
- **Failure:** a failed promotion keeps the old binding.
- **Repeating:** repeating a promotion that already happened does nothing.
- **Old rows:** they are removed only when the stream's last reference to them
  goes. There's no unbind command.

## Staged rebuilds

For models larger than the atomic cap, build an inactive revision while the old
one keeps serving:

```bash
cantongrid models rebuild start  .local/runtime.json records-v2 demo records 2 --replace 1
cantongrid models rebuild step   .local/runtime.json records-v2 --limit 32   # repeat until phase=ready, has_pending=false
cantongrid models rebuild activate .local/runtime.json records-v2
cantongrid models rebuild step   .local/runtime.json records-v2 --limit 32   # repeat until phase=complete
cantongrid models rebuild cancel .local/runtime.json records-v2              # before activation only
```

- **Phases:** `cleaning`, then `backfill`, then `catchup`, then `ready`.
  - **Catchup:** reconciles creates and archives that arrived between steps.
  - **Persistence:** progress survives restarts, and ingestion keeps running
    between steps.
- **Activation:**
  - **Refusal:** it refuses if changes arrived after `ready`. Step again first.
  - **Switch:** it switches the binding under the writer lock, and model cursors
    restart.
  - **Partial rows:** they're never queryable.
- **After a replacement:** the job enters `cleanup`, so keep stepping.
  - **Rolling back:** start a new job aimed at the old revision.
  - **Cancelled jobs:** they can't restart.
- **Limits:**
  - **Steps:** each step handles 1–64 contracts and 8 MiB. Oversized work rolls
    back, so retry with a smaller limit.
  - **Concurrency:** at most 16 unfinished jobs and 16 active bindings per source.
  - **Active models:** an active fingerprint can't be rebuilt in place.
- **Checks:** every action re-checks source rights, readiness and the plan.
  - **Busy lock:** retry later.
  - **Bad expressions:** a bad staged expression rolls back only that job's page.
- **Status:**
  - **Counters:** it reports string counters, never payloads.
  - **History:** there's no automatic retention of job history.

## Aggregates

Since 0.4.0, a model can declare grouped counts and sums:

```json
"aggregation": { "group_by": ["status"],
  "measures": { "markets": { "op": "count" }, "total": { "op": "sum", "field": "amount" } } }
```

- **Shape:** `group_by` takes 1–4 projected fields and `measures` takes 1–8
  `count` or `sum` measures. Optional fields can't be grouped or summed.
- **Computation:** results are computed in PostgreSQL from the current projection
  rows.
  - **No cache:** there's no separate totals cache.
  - **Lifecycle:** a create contributes once, an archive retracts, and a replay
    changes nothing.
- **Access:** the caller must hold **every configured source party**, otherwise
  REST returns 403, even for a group made only of its own inputs.
  - **Why:** this stops a group's disappearance from leaking membership.
  - **Group visibility:** a group is visible only when every contributor is.
- **Numbers:**
  - **Counts:** exact strings.
  - **Sums:** Int64 sums widen to Numeric(38,0), and Numeric sums keep their scale.
  - **Overflow:** it fails the query.
- **Query:**
  - **REST:** `POST /v1/models/{ns}/{name}/aggregate` with `{"limit", "cursor"}`.
  - **GraphQL:** the generated `aggregateQueryField`.
  - **Items:** each item has an opaque `groupKey` and typed `values`.
  - **Not yet:** filters, sorting and joins on aggregates.
  - **Errors:** any resolver error clears the whole GraphQL `data`.
- **Examples:** `examples/models/aggregate-outcomia.json` and
  `aggregate-visibility.json`.
- **LocalNet check:** `demo.py aggregate-check` compares the results with a
  ledger snapshot, read-only.
- **Fingerprint:** adding an aggregation changes the fingerprint, so make a new
  revision. Upgrade every worker and API first, because 0.3 rejects it.

Relationships and joins between models are in [composition](composition.md).
