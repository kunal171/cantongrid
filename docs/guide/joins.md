# Typed inner joins

M4 is available in the source checkout preparing **0.4.0-alpha.4**. It returns
selected typed fields from independently visible contracts linked by an explicit
[relationship declaration](relationships.md). It does not infer ancestry.

## Enable

Add a projection to a registered relationship in a new model revision:

```json
"join": {
  "kind": "inner",
  "source_fields": ["factoryKey"],
  "target_fields": ["label"]
}
```

Each side selects 1–16 unique existing model columns. Only inner equality joins
are supported. Both keys must have the same exact Text, Party, Int64 or Numeric
type/scale. Selected output values may also be Boolean or optional/null.
Joined Int64/Numeric values remain strings. No caller can add output fields,
change keys, choose SQL or request an outer join.

The complete [example](../../examples/joins/README.md) has offline preview, SQL
planning, GraphQL and TypeScript commands. Include both model definitions in the
generation catalog so target field types can be resolved. For runtime use, verify actual package
metadata and bind both models in the same namespace/physical source plan. The
pinned target revision must be active. Existing [model publication](models.md)
and rollback commands apply; no new dependency or migration was added.

Definitions without `join` retain their old fingerprints and expose no join root.
M3 relationship queries continue to return IDs/provenance only. A changed column
selection requires a new model revision and explicit publication. Upgrade workers
and API instances together; older binaries reject the new vocabulary.

## Query

```http
POST /v1/models/demo/factories/joins/declaredChildren/query
Authorization: Bearer <organization-token>
Content-Type: application/json

{"contract_id":"<visible-factory-contract-id>","limit":10}
```

Discover the generated field with `GET /v1/models` (`joinQueryFields`) or the
`models graphql` command. Its name includes both model fingerprints, so different
target definitions in separate namespaces cannot share an incompatible type.
On `/models/graphql`:

```graphql
query JoinedChildren($namespace: String!, $root: String!, $cursor: String) {
  REPLACE_WITH_GENERATED_JOIN_FIELD(
    namespace: $namespace
    contractId: $root
    limit: 10
    cursor: $cursor
  ) {
    visibleTotal
    nextCursor
    fingerprint
    items {
      sourceContractId
      targetContractId
      declaration
      declarationRevision
      sourceFingerprint
      targetFingerprint
      relationship
      joinKind
      coverage
      depth
      sourceValues { factoryKey }
      targetValues { label }
    }
  }
}
```

Results are `relationship: "declared"`, `joinKind: "inner"`,
`coverage: "current_active_projection"`, depth 1. Only the selected model values
are returned; raw payloads, witnesses and unselected fields remain absent.

## Visibility, paging and updates

Both active contracts and model rows must independently be visible in the same
read snapshot. A-only source plus B-only target yields no combined row to A;
B cannot query A's root; a caller with both legitimate grants can see the join.
Unknown, filtered, archived and hidden roots share the same 404. Hidden targets
produce no count, unmatched-root placeholder or indication of their existence.
Aggregate endpoints retain their existing complete-source scope requirement.

Limit is 1–25 (default 20), with the declaration's maximum 100 visible targets.
Selected values across the complete visible result must fit 2 MiB; the response
also has a 2 MiB cap, including repeated source values and JSON encoding. An
oversized result fails rather than truncating fields. Hidden values do not consume
these budgets. At most one page of selected target values is materialized in Rust.

Cursors bind the operation, root, both fingerprints, namespace, generation,
caller policy and visible selected result. They work across REST/GraphQL for the
same join. Visible creates/archives or changed selected values invalidate them:
restart with no cursor. Hidden updates do not invalidate a caller's cursor.
Replay and failed projection transactions do not change the visible result.
A relationship/model cursor cannot be reused as a join cursor. Revocation,
source unavailability and expired source leases stop delivery.

This is current-state, root-scoped, one-hop composition. Global joins, outer/anti
joins, transitive traversal, historical queries and public streams remain outside
M4. See the [contract](../design/joins.md) and [verification](../verification.md).
