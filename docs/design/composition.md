# Design: relationships, joins, inspector and impact

These are the contracts behind milestones M3–M6 (all in 0.4.0).
- **Usage:** [composition](../guide/composition.md),
  [operations](../guide/operations.md#inspector) and
  [upgrades](../guide/upgrades.md).
- **What they don't need:** a new runtime dependency.
- **Migrations:** M3–M6 reuse migration 010.

## Common privacy rules

- **Two checks:** every endpoint contract, and every model row it relies on, must
  be independently visible in the same stream and the same repeatable-read
  snapshot.
- **Order of filtering:** hidden rows are removed **before** counting, sorting,
  fanout checks, paging and encoding.
  - **Hidden roots:** a hidden root looks exactly like an unknown one.
  - **No hints:** hidden targets leave no placeholders or counts.
- **Cursors:** signed cursors are bound to the operation, the root, the
  fingerprints, the namespace, the generation and the caller policy.
  - **Not an oracle:** a global checkpoint is never used as a cursor oracle.
- **Shared guards:** these surfaces reuse the API's source health, current grants,
  resolver admission, binding and schema checks, and final encoding guards.
- **Paging:** 1–25 items (default 20).

## M3: declared relationships

- **Declaration:** a model declares named, revisioned outgoing links. Each one
  names a projected source field and a target model, revision and field in the
  same namespace.
  - **Key types:** both must have the same exact type (Text, Party, Int64 or
    Numeric).
  - **Not keys:** optional and Boolean fields, expressions, SQL and fuzzy matches.
- **Limits:** 8 declarations per model, and 100 *visible* targets per root, so
  hidden rows can't turn a success into a limit error.
- **Fingerprint:** declarations are part of the model fingerprint, and old catalogs
  keep identical fingerprints.
- **What it never claims:** links are labelled `declared` and are never source
  proofs. Self-links are excluded, and uniqueness and absence are never claimed.
- **Acceptance:**
  - **Declarations:** strict ones, with revision drift rejected.
  - **Visibility matrix:** an A/B/shared matrix on owned PostgreSQL.
  - **Numbers:** exact large keys.
  - **Robustness:** cross-stream isolation, archive and recreate with a stable key,
    replay, projection failure, promotion and revocation.
  - **Transports:** REST, GraphQL, SDL and TypeScript all agree.

## M4: safe inner joins

- **Declaration:** an optional `join` on a relationship selects 1–16 fields from
  each model. Its kind is `inner`.
  - **Fingerprint:** it's part of the fingerprint. Models without joins keep their
    M3 fingerprints.
- **Results:**
  - **Scope:** one root and at most 100 visible targets.
  - **Values:** typed `sourceValues` and `targetValues`.
  - **Never included:** raw payloads, caller-chosen columns, outer joins,
    anti-joins and multi-hop joins.
- **Cursor invalidation:**
  - **Binding:** cursors also bind a digest of the caller-visible selected result.
  - **Visible changes:** they invalidate the cursor.
  - **Hidden changes:** they must not.
  - **Digest:** only permitted values are hashed, and the digest stays inside the
    signed cursor.
- **Budget:** one page of values in memory, with 2 MiB for the selected result and
  for the response.
- **Cursor domain:** relationship cursors aren't valid as join cursors.
- **Acceptance:**
  - **One side visible:** an A-only source with a B-only target gives A nothing,
    and B can't query the root.
  - **Both visible:** a shared caller sees both values.
  - **Hidden and empty:** hidden and no-match results look the same.
  - **Owned PostgreSQL only:** no LocalNet run is claimed.

## M5: developer inspector

- **Transaction view:** a flat view of M2 creation and exercise evidence, plus the
  matching archives.
  - **Ordinals:** numbered **after** visibility.
  - **Never included:** raw node IDs, bounds, controllers, arguments, payloads,
    witnesses, URLs or SQL.
  - **Nesting:** none is guessed. Proven edges come from the lineage API.
  - **Coverage:** always partial. A hidden descendant is never flagged.
  - **Limits:** 128 records and 2 MiB.
- **Projection diagnosis:**
  - **What it compares:** one visible contract against the current bound revision,
    natively in SQL.
  - **Output:** a status and finite codes, never values.
  - **Drift:** reported, never repaired.
- **`inspect evaluate`:** evaluates a supplied payload and emits only identity,
  status and codes. It reads no logs and doesn't re-run commands.
- **`inspect progress`:** renews source authority, then reads restricted bounds and
  owned metadata without advancing anything.
  - **Boundaries:** source and destination are separate observations, not a lag.
  - **Failure history:** failed operations aren't stored, and the report says so.
- **Acceptance:** the visibility matrix, cursors, source outage and expiry,
  reconciliation of create, exercise and archive at the same offset, drift,
  rollback, and no secrets in diagnostics.

## M6: upgrade impact analysis

- **Report section:** a versioned `impact` section in the compatibility report.
  For each model:
  - **Field reads:** old and new LF semantics and scalar types.
  - **Dependencies:** downstream filter, aggregate, relationship and join
    dependencies.
  - **Identities:** current and proposed fingerprints, the table and API
    identities.
  - **Rebuild class:** `unnecessary`, `required` or `blocked`.
- **Fail-closed:** removing a field, or changing optionality, a Numeric scale, an
  enum or a ContractId target, blocks the bundle. Renames aren't guessed, and
  pins aren't rewritten.
- **Sample evaluation:**
  - **What runs:** projection expressions only, with sanitized M5 diagnostics.
  - **What doesn't:** LF interpretation, and source or storage access.
- **What it never does:** planning never touches a ledger or database, and nothing
  resets checkpoints, deletes history, changes grants or uploads a DAR.
- **Acceptance:** deterministic ordering, unselected changes, pin drift, missing
  targets, rejected bundles, fixture limits and redaction.
