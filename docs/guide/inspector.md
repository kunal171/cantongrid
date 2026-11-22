# Developer inspector

Available in the source checkout preparing **0.4.0-alpha.5**. Existing migrations
001–010, source plans and model fingerprints are unchanged. No reindex is needed.
The [example](../../examples/inspector/README.md) runs offline diagnostics and the
owned PostgreSQL tests.

## Committed transaction records

With the ordinary organization bearer token, request:

```http
GET /v1/inspect/transactions/<transaction-id>?limit=10
Authorization: Bearer <organization-token>
```

On the generic `/graphql` endpoint:

```graphql
query InspectTransaction($transaction: String!, $cursor: String) {
  inspectTransaction(transactionId: $transaction, limit: 10, cursor: $cursor) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

This requires the explicit M2 lineage profile and its observed forward creation/
exercise metadata. Records identify `kind`, `contractId`, `template` and a visible
`ordinal`; exercises additionally include `choice` and `consuming`. A correlated
canonical archive is listed alongside its consuming exercise. Ordering is source
node order with local kind ordering, not a simulation of ledger execution time.

Both source-event witnesses and the independently indexed contract must permit
the caller. Archived indexed contracts remain inspectable under current grants.
Unindexed transient/divulged endpoints are omitted. Unknown and hidden
transactions share one 404 response. Rejected submissions are not reconstructed.

Ordinals and totals are assigned after visibility filtering. Raw node numbers,
subtree bounds, controllers, arguments/results, payloads and witness lists are
absent. Proven nesting remains available through the separate guarded
[lineage API](lineage.md); the inspector never guesses a tree through missing nodes.
Coverage always says `partial_recorded_evidence`, `complete: false`, with a flat
authorized view. It does not signal hidden descendants with a special coverage flag.

Page size is 1–25 (default 20), bounded to 128 visible records and a 2 MiB response.
Signed cursors bind transaction, operation, physical plan/generation and caller
policy. REST/GraphQL cursors are interchangeable for the same transaction. No
source watermark or private source-node position appears in the cursor/page.

## Model/projection consistency

For one current bound model and an independently visible contract:

```http
POST /v1/models/demo/records/inspect
Authorization: Bearer <organization-token>
Content-Type: application/json

{"contract_id":"<visible-contract-id>"}
```

Discover `diagnosticQueryField` through `GET /v1/models`, or generate the SDL and
TypeScript with the existing `models graphql` / `models typescript` commands.
On `/models/graphql`, substitute that generated field:

```graphql
query InspectProjection($namespace: String!, $contract: String!) {
  REPLACE_WITH_DIAGNOSTIC_FIELD(namespace: $namespace, contractId: $contract) {
    items {
      contractId
      namespace
      model
      revision
      fingerprint
      active
      status
      errorCode
      projection
      coverage
    }
    visibleTotal
    nextCursor
    fingerprint
  }
}
```

The single-item report has no cursor. `status` is `matched`, `filtered` or `invalid`,
with a finite error code for invalid input. `projection` is `consistent`, `missing`,
`unexpected`, `different` or `evaluation_failed`. Archived/filtered contracts
should have no current model row. Values are compared natively in PostgreSQL;
Int64/Numeric never round through JSON numbers. The report includes no values.

Checks use one read-only snapshot and validate the active binding and installed
schema. The final transport guard rechecks catalog, source and caller authority.
Schema drift makes the request unavailable; data drift is a finding, never an
automatic repair. Publishing a revision changes the generated field and reported
revision. Aggregate group data and staged target rows are not exposed.

## Local operator evidence

```bash
cantongrid inspect evaluate models.json records private-input.json
cantongrid inspect progress .local/runtime.json
```

`evaluate` reads only operator-supplied files. It exports model identity, status
and an error code; it does not execute a ledger command. `progress` renews current
source authorization/health, reads count-free bounds through the restricted PQS
reader, and reads owned checkpoint/model/rebuild metadata without advancing it.
There is no public progress API.

Progress separates source and destination observations. `boundaryDifference` is
an exact difference in source-local offsets, not an event count or elapsed time.
The observations are not atomic across databases and do not prove complete history.
Private checkpoint PKs, scan keys, raw SQL, credentials and source user IDs are
excluded. Existing failed operations have no durable trace; the report labels
that `not_recorded` rather than inventing a failure history.

JSON stdout or an authorized API response can be redirected into an evidence
file. Source payloads and logs are never bundled automatically. See the
[design](../design/inspector.md) and [verification](../verification.md) for fixture,
retained-source and live-authorization distinctions.
