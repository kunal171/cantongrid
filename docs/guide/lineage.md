# Direct contract lineage

Available in the source checkout preparing **0.4.0-alpha.4**. The published
alpha.3 receipt profile has no lineage API.

## Enable

Prepare a restricted schema041 tree-mode source as described in
[LocalNet ingestion](localnet-source.md). Use a new explicit source generation
and add both runtime fields:

```json
{
  "exercise_source": "pqs041_ledger_effects",
  "lineage_source": "pqs041_complete_subtrees"
}
```

These fields extend the normal runtime configuration. Run explicit
`cantongrid storage init --database-env CANTONGRID_DESTINATION_URL` to apply
migration 010 before starting the new worker/API. Migrations 001–009 are unchanged.
An alpha.3 binary cannot open 010; rollback requires its pre-upgrade destination
backup. Do not edit migration history to downgrade.

Creation provenance starts after the new generation's snapshot. Existing receipt
history is not silently reinterpreted or backfilled. Source and caller rights
are checked independently; model fields cannot grant access.

## Query

REST: `GET /v1/contracts/{contractId}/lineage?direction=children&limit=10`.
Direction is `parents` or `children`, with one hop per request. Limit is 1–25
(default 20). Supply the ordinary organization bearer token. Unknown and
unauthorized roots return the same unavailable response.

```graphql
query ContractLineage($contract: String!, $cursor: String) {
  contractLineage(
    contractId: $contract
    direction: "children"
    limit: 10
    cursor: $cursor
  ) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

Each item identifies its parent/child contracts and templates, choice and
source-local offset/node IDs. It is labelled `observed_direct` with a
`complete_source_subtree` proof. Witness lists, payloads, hidden endpoints and
internal proof-node lists are not returned. Counts and pagination include only
permitted edges. Cursors bind the root, direction, caller policy and source plan.

The caller must independently access both indexed endpoint contracts and every
proof input. Observing a protocol/factory does not grant access to its children.
Caller revocation, source unavailability and expired leases stop delivery.

Coverage is always `partial_recorded_evidence`, with `complete: false` and
`depth: 1`. An empty page does not establish absence of a relationship. Missing
nodes or unknown bounds produce no guessed edge; a matching application ID is
not proof. See the [proof contract](../design/lineage.md) for the conservative
subtree criterion and [verification](../verification.md) for exact tested scope.

For explicit application links, use the separate [declared relationship API](relationships.md).
Joins, transitive traversal, public streams, interface choices and receipt/lineage
package handoff remain later work.
