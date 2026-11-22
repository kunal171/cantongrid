# Declared relationships

M3 is available in the source checkout preparing **0.4.0-alpha.4**. A relationship
is an explicit application policy over model fields. It is always labelled
`declared`; it does not prove that a factory created a contract. [Direct lineage](lineage.md)
uses separate source evidence and never consumes these declarations.

## Define and generate

The [example](../../examples/relationships/README.md) maps the generic Daml
fixture's `Factory.factoryId` to `Child.declaredFactory`. On the source model:

```json
"relationships": [
  {
    "name": "declaredChildren",
    "revision": 1,
    "source_field": "factoryKey",
    "target": {"model": "children", "revision": 1, "field": "factoryKey"},
    "max_targets": 100
  }
]
```

Each side names a projected field, not a payload path or SQL expression. Keys
must have the same exact Text, Party, Int64 or Numeric type/scale. Optional and
Boolean keys are rejected. At most eight declarations per model and 100 visible
targets per root are allowed. Self-links to the root contract are excluded.

Generate ordinary model SQL plans and typed interfaces with the existing commands:

```bash
cantongrid models validate examples/relationships/models.json
cantongrid models plan examples/relationships/models.json
cantongrid models graphql examples/relationships/models.json
cantongrid models typescript examples/relationships/models.json
```

The checked-in package IDs are offline placeholders. For live ingestion, replace
them with verified metadata, select both templates in runtime configuration, then
index the authorized source. Register, install and bind both models in the same
namespace and source plan using the [model workflow](models.md). Targets included
in one catalog are checked immediately; separately registered targets are checked
against their installed, active revision at query time. No lineage opt-in or new
migration beyond the checkout's existing migration 010 is required.

## Query

REST accepts only a root and page parameters:

```http
POST /v1/models/demo/factories/relationships/declaredChildren/query
Authorization: Bearer <organization-token>
Content-Type: application/json

{"contract_id":"<visible-factory-contract-id>","limit":10}
```

Discover the generated GraphQL field from `relationshipQueryFields` in
`GET /v1/models` or from `models graphql`. Use it on `/models/graphql`:

```graphql
query DeclaredChildren($namespace: String!, $root: String!, $cursor: String) {
  REPLACE_WITH_GENERATED_FIELD(
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
      relationship
      declaration
      declarationRevision
      sourceModelRevision
      targetModelRevision
      sourceFingerprint
      targetFingerprint
      coverage
      depth
    }
  }
}
```

Items contain endpoint IDs and version provenance. They do not return keys,
joined payloads, witnesses or source-proof claims. Coverage is
`current_active_projection`, depth is 1. Limit is 1–25 (default 20).

Both active contracts and both maintained model rows must be independently
visible. Unknown, archived, filtered and hidden roots return the same 404.
Counts, ordering, fanout checks and pagination exclude hidden targets. Cursors
bind root, declaration, both fingerprints, namespace, generation and caller
policy. REST and GraphQL cursors are interchangeable for the same query.

## Updates and revision changes

Creates and archives update both projections in the existing atomic ingestion
transaction. A successor with the same application key can match the policy;
this does not assert continuity or uniqueness. Reads start from current observed
state, including the snapshot; they do not provide historical relationship edges.

Definitions are part of immutable model fingerprints. Change a declaration only
in a new model revision, then register/install and explicitly publish it. A target
revision change makes the old declaration unavailable until a compatible source
revision is published or the target is rolled back. Existing source/target
publication and rollback commands apply; there is no automatic policy widening.
Old cursors are rejected after policy or grant changes.

Upgrade all workers and API instances before publishing declarations; older
binaries reject the new model vocabulary. No new third-party dependency was added.
Read the [design](../design/relationships.md) and [verification](../verification.md)
for tested scope. Incoming/transitive traversal, historical queries, joined values
and durable streams remain separate milestones.
