# Safe inner joins (M4)

Status: implemented on `feat/safe-inner-joins`, based on completed M3.
Owned PostgreSQL and mock-source API acceptance passed; no new LocalNet run claimed.

## Definition and scope

An optional `join` projection on a registered relationship explicitly enables
joined rows. It names `kind: "inner"` and selected `source_fields`/`target_fields`
from the two registered models (1–16 unique fields each). The existing equality
key types and target revision pins apply. Boolean/optional projected values are
allowed; optional/Boolean equality keys remain unsupported. The declaration is
part of the immutable model fingerprint. Absent join declarations preserve M3
fingerprints and expose no join root. No new dependency or migration is needed.

This MVP resolves one root and at most 100 visible matching targets. It returns
endpoint/declaration provenance plus typed `sourceValues` and `targetValues`.
Int64/Numeric remain exact strings; bool and optional null keep their types.
These are application-declared inner joins, never source ancestry proofs.
No raw contract payload, arbitrary SQL, caller-selected columns, outer join,
anti-join, multi-hop composition or aggregate-policy change is included.

## Privacy, pagination and budgets

Both canonical active contracts and both model rows must independently be
visible in the same physical stream and repeatable-read snapshot. Missing,
filtered, archived and hidden roots share the unavailable response. Hidden
matches are excluded before count, sort, fanout and value encoding. No unmatched
root placeholder or indication of a hidden counterpart is returned.

Page size is 1–25/default20. In addition to both fingerprints, root, declaration,
namespace, generation and caller policy, join cursors bind a digest of the
caller-visible selected result. Archive/recreate or value changes to that result
invalidate old pages. Hidden updates must neither invalidate a cursor nor change
its visible digest. Do not use a global checkpoint/revision as a cursor oracle.
Hash only permitted selected values, keep hashes internal/inside signed cursors,
and bound metadata/result processing. Materialize at most one requested page's
values in Rust; enforce a 2 MiB selected-result and response budget.

REST and generated GraphQL use a join-specific operation domain and the model
APIs' source health, current grants, resolver admission, binding/schema checks and
final encoding guards. A relationship cursor cannot be used as a join cursor.

## Acceptance

- Exact typed declarations; reject missing, duplicate or unknown columns, bad
  join kinds and unsafe keys. Old catalogs retain identical fingerprints.
- Source A-only + target B-only: A gets no combined row, B cannot query the root,
  shared caller gets both selected values. Hidden/unknown and hidden/no-match
  results are indistinguishable; no outer-placeholder inference.
- Visibility before counts/paging/fanout/encoding, exact Numeric/Int64, bool/null,
  selected columns only, deterministic native-ID order and bounded output.
- Visible archive/recreate invalidates cursors; hidden updates do not. Exercise
  replay, projection rollback, model publication/rollback and schema drift.
- REST/GraphQL parity, signed cursor domains, caller/source revocation, response
  size and shared resolver limits; generated SDL and strict TypeScript agree.
- Runnable example, guide and evidence labelled owned PostgreSQL/mock-source
  tests. This work does not imply LocalNet or production acceptance.
