# Declared relationships (M3)

Status: implemented on `feat/declared-relationships`, based on M2; owned
PostgreSQL and mock-source API acceptance passed. No new LocalNet run claimed.

## Contract

A model can declare named, revisioned outgoing relationships. Each declaration
names one projected source field and a target model/revision/field in the same
namespace. Both exact scalar types must agree. Text, Party, Int64 and Numeric
are supported; optional, Boolean, arbitrary expressions, SQL and fuzzy matches
are not relationship keys. Maximum eight declarations per model and 100 visible
targets per root. Definitions are part of the existing immutable registered model
fingerprint; changing one requires a new model revision and explicit publication.
No new runtime dependency or destination migration is required.

The engine resolves equality against current, active, maintained projections in
one repeatable-read snapshot. Target revision must be actively bound in the same
source plan/generation. Installed schemas and both binding fingerprints are
verified. Missing, stale or incompatible targets fail closed. Ordinary lifecycle
and model publication atomically update the inputs; no independent graph cache
can drift or invent history. Stable application keys can relate contracts across
transactions and successor contract IDs; historical/as-of relationship queries
remain outside this slice.

Results are always `declared`, never `observed_direct` or source-proven ancestry.
They contain endpoint IDs and declaration/model revision provenance, not joined
payloads or key values. M2 lineage stays separate and never consumes declarations.
A forged field can satisfy an explicit application policy but cannot become proof.

## Authorization and API

Both canonical endpoints must independently be active and visible, with both
model rows present in the exact configured stream. Filter before count, ordering,
fanout checking and keyset paging. Hidden and unknown roots have the same
unavailable response. Hidden targets produce no placeholders or hidden counts.
Self-links are excluded. No target uniqueness or absence is claimed. The 100-target limit applies only to
visible rows, so hidden rows cannot change success into a limit error.

Expose one outgoing hop through authenticated REST and generated model GraphQL.
Bind signed cursors to namespace, source/target fingerprints, declaration, root,
caller policy and generation. Reuse source health, current grants, bounded worker
admission, final schema/binding checks and encoding guards. Limit 1–25/default20;
no incoming traversal or caller-supplied relationship policy.

## Acceptance

- Reject unknown fields, bad names, duplicate declarations, wrong/missing key
  types and target revision drift; preserve old catalog fingerprints.
- Owned PostgreSQL A/B/shared matrix: hide before counts/paging, missing-root
  equivalence, bounded fanout, exact large numeric keys, cross-stream isolation.
- Cross-transaction creation, archive/recreate with a stable key, replay,
  atomic projection failure, binding promotion and revocation/cursor controls.
- REST/GraphQL agree; generated SDL and TypeScript describe the same interface.
- Runnable generic configuration example and guide. Existing fixture evidence is
  not labelled LocalNet acceptance; no new ledger-write authorization is assumed.

[M4 inner joins](joins.md) add an explicit selected-value projection; this M3 API
retains IDs/provenance only. Transitive traversal, historical materialization and
streams remain separate milestones. No production acceptance claim.
