# Direct lineage (M2)

Status: implementation in progress on `feat/privacy-safe-lineage`.
Baseline: release commit `3ab2715547571383fcb1b8d40047212c14959229`.

## Evidence contract

M1 stores exercise receipts, not typed creation-node evidence. M2 adds an explicit
`lineage_source: "pqs041_complete_subtrees"` profile alongside exercise ingestion.
It changes the durable plan definition: use a new generation, never silently
adopt a receipt-only checkpoint. Snapshot history stays incomplete.

Retain selected forward creation metadata, including transient/divulged nodes,
without adding them to contract state. Source node identity is scoped by stream,
offset and node ID; update ID and executed/creation package identities remain exact.

An edge is direct only when its containing exercise's entire numeric node interval
is present, unique and consistently nested, with known subtree bounds. Each node
must be a supported creation/exercise from the same scoped transaction. Choose the
innermost containing exercise only after those checks. Gaps, unknown bounds,
crossing intervals or unsupported source shapes cannot prove direct parentage.
The consecutive-ID criterion is deliberately conservative; it is not a claim that
every unfiltered transaction has consecutive exported IDs.

This follows the pinned profile's observed IDs and the official
[Ledger Effects subtree semantics](https://docs.digitalasset.com/build/3.5/reference/json-api/openapi.html).
The API preserves ancestry under filtering; direct parentage requires sufficient
evidence. Application field values, timestamps and choice names are never proof.

## Storage and query

- Migration 010 adds private creation/page evidence, direct edges and an internal
  edge outbox. Commit them with the existing lifecycle/model journal and checkpoint.
  Migrations 001–009 remain immutable. No new third-party dependency.
- Bound transaction provenance to 64 nodes; preserve the existing 64 lifecycle/
  exercise-record budget and 1 MiB combined delivery budget. Reject oversize input.
- Each edge retains an AND of proof-node witness obligations (OR within each node).
  Reads additionally require current visibility of both indexed endpoint contracts.
  An exercise observer gains no contract access through an edge.
- A missing/unindexed/unauthorized root has one indistinguishable unavailable
  response. For a visible root, hidden or unproven edges leave no IDs, counts or
  placeholders. Coverage always reports partial recorded evidence; do not label
  a visible root incomplete because a particular hidden edge exists.
- REST `/v1/contracts/{contractId}/lineage` and GraphQL `contractLineage` expose
  one-hop `parents`/`children` pages only. Limits, signed cursors, source health,
  independent caller grants and final encoding guards match existing APIs.
  Bind cursors to root, direction, scope/generation and current caller policy.

## Acceptance

Proof controls: pair, nested, transient, forged reference, missing/unknown nodes,
crossing intervals, mixed identities and bounds. Storage controls: replay,
conflicts, model/SQL rollback, late lease expiry, schema/ACL drift and old-profile
rejection. API matrix: A, B, shared, ungranted and revoked callers; hide before
count/pagination and bind cross-root/direction cursors. Compare authorized LocalNet
queries against the existing generic fixture. Reuse only previously approved
local controls; no new DAR/party provisioning or Outcomia modification is implied.

Declared relationships (M3), joins (M4), general traversal, inspector and durable
public streams remain deferred. No production-readiness claim.
