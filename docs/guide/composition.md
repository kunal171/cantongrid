# Lineage, relationships, joins and history

There are four ways to read across contracts, from the strongest evidence to the
weakest.

| Feature | Evidence | Since |
| --- | --- | --- |
| [Direct lineage](#direct-lineage) | Observed in the source's transaction tree | 0.4.0 |
| [Declared relationships](#declared-relationships) | An application policy on model keys | 0.4.0 |
| [Typed inner joins](#typed-inner-joins) | Relationships, plus selected values | 0.4.0 |
| [Historical state](#historical-state) | Retained create and archive events (experimental) | 0.4.0 |

**Shared rules.**
- **Two checks:** every read checks source rights and caller grants separately.
- **Hidden endpoints:** they are never counted, returned or hinted at, and
  unknown or hidden roots return the same response.
- **Paging:** 1–25 items per page (default 20).
- **Cursors:** bound to the query and the caller's policy, and shared between REST
  and GraphQL for the same query.

The contracts behind these guarantees are in
[design: composition](../design/composition.md).

## Direct lineage

Enable it with a new source generation and these runtime fields:

```json
{ "exercise_source": "pqs041_ledger_effects", "lineage_source": "pqs041_complete_subtrees" }
```

**Migration.**
- **Apply it:** run `cantongrid storage init` to apply migration 010.
- **No downgrade:** an alpha.3 binary can't open 010, and rolling back needs the
  pre-upgrade backup.
- **Coverage starts at the snapshot:** provenance begins at the new generation's
  snapshot. Old receipts aren't backfilled.

**Querying.**
- **REST:** `GET /v1/contracts/{id}/lineage?direction=parents|children&limit=10`.
- **GraphQL:** `contractLineage(contractId, direction, limit, cursor)`.
- **Depth:** one hop per request.
- **What each item contains:** parent and child contracts and templates, the
  choice, and source offsets and node IDs, labelled `observed_direct` with a
  `complete_source_subtree` proof.
- **What it leaves out:** witnesses, payloads and proof nodes.
- **Access:** the caller must see both endpoints and every proof input. Seeing a
  factory doesn't grant access to its children.

**Coverage** is always `partial_recorded_evidence`, `complete: false`, depth 1.
- **Empty pages:** an empty page doesn't prove there's no link.
- **No guessed edges:** missing nodes never produce a guessed edge, and a
  matching application ID isn't proof.

## Declared relationships

A relationship links projected model keys. It is labelled `declared` and doesn't
prove that one contract created another.

```json
"relationships": [{ "name": "declaredChildren", "revision": 1, "source_field": "factoryKey",
  "target": { "model": "children", "revision": 1, "field": "factoryKey" }, "max_targets": 100 }]
```

**Keys.**
- **Types:** both keys must be the same exact type (Text, Party, Int64 or Numeric
  with scale). Optional and Boolean keys are rejected.
- **Limits:** up to 8 declarations per model and 100 visible targets per root.
- **Self-links:** they're excluded.

**Setup.**
- **Models:** register and bind both models in the same namespace and source
  plan.
- **Revisions:** declarations are part of the model fingerprint, so changing one
  needs a new model revision.
- **Upgrades:** upgrade every worker and API before publishing.

**Querying.**
- **REST:** `POST /v1/models/{ns}/{model}/relationships/{name}/query` with
  `{"contract_id", "limit"}`.
- **GraphQL:** the generated field listed in `relationshipQueryFields`.
- **What it returns:** endpoint IDs and version provenance, with coverage
  `current_active_projection` and depth 1. No keys or payloads are returned.
- **Access:** both contracts and both model rows must be visible.
- **Current state only:** there are no historical edges.
- **Target changes:** a target revision change makes the declaration unavailable
  until a compatible source revision is published.

## Typed inner joins

A join adds selected values to a relationship, in a new model revision:

```json
"join": { "kind": "inner", "source_fields": ["factoryKey"], "target_fields": ["label"] }
```

**Definition.**
- **Fields:** each side selects 1–16 model columns. Values may be Boolean or
  optional, and numbers stay strings.
- **Fixed by the definition:** callers can't add fields, change keys, choose SQL
  or request an outer join.

**Querying.**
- **REST:** `POST /v1/models/{ns}/{model}/joins/{name}/query`.
- **GraphQL:** the generated field listed in `joinQueryFields`. Its name includes
  both fingerprints.
- **What it returns:** `sourceValues` and `targetValues`, with `joinKind: "inner"`.

**Access.**
- **Both sides:** the caller must see both sides in the same snapshot.
  - **One side only:** an A-only source and a B-only target show nothing to A.
  - **Both grants:** a caller with both grants sees the join.

**Size limits.**
- **Selected values:** at most 2 MiB across the visible result.
- **Response:** at most 2 MiB.
- **Oversized results:** they fail rather than being truncated.

**Cursors.**
- **Invalidation:** a visible create, archive or value change invalidates the
  cursor, so restart without one.
- **Hidden updates:** they don't invalidate it.

**Scope:** current state only, one hop from a root. There are no global, outer or
transitive joins.

## Historical state

An experimental prototype rebuilds contract membership at a source offset from
retained create and archive events, under the caller's current grants.

**Querying.**
- **REST:** `GET /v1/contracts/as-of?asOfOffset=100&limit=20`.
- **GraphQL:** `contractsAsOf(asOfOffset, limit, cursor)`.
- **Offsets and counts:** decimal strings.

**Coverage.**
- **Success:** `coverage: observed_retained` and
  `completeWithinConfiguredScope: true`, from `historyFromOffset` onward.
  Additive package baselines can raise that floor.
- **Unavailable:** HTTP 409 (or a GraphQL error) with `coverage: unavailable`. This
  happens before the snapshot, for unacknowledged offsets, while bootstrapping, or
  when history is missing. The API never returns a confident zero.

**Budgets.**
- **Per query:** 4,096 journal pages, 8,192 events and 64 package baselines.
- **Over budget:** the result is unavailable rather than truncated.

**Not covered:** historical model projections, aggregates and joins, as well as
wall-clock and application-time queries.
