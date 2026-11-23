# Reconciliation policies: offline first slice

Next-track proposal after stable 0.4.0, based on T8 in the supplied roadmap.
This first slice compiles versioned arithmetic comparison policies and evaluates
explicit local fixtures. It adds no runtime dependency, database migration,
network access, source grant, ledger write or public API.

## Policy and evaluator

A version-1 document contains one named positive-revision policy, 1–32 named
expressions and 1–16 uniquely named checks. Reuse the existing model expression
compiler and checked arithmetic; allow only required Int64/Numeric outputs.
Compare two named expressions using eq/ne/lt/lte/gt/gte. Numeric operands must
have the same declared scale. No rounding, optional defaults, approximate tests,
implicit zero, arbitrary SQL or executable plugins.

Use the existing 64 KiB document/payload, depth-16 and 256-total-expression-node
budgets. Strict JSON rejects duplicate keys, unknown schema fields and floating
point tokens. Canonical policy fingerprints include revision and definitions;
map/check ordering and whitespace do not change the fingerprint.

Examples: exact collateral-vs-liability comparison and unrelated synthetic supply
conservation. They are arithmetic fixtures, not claims about deployed Outcomia
contracts or actual money. Asset/unit correctness remains a developer policy
assumption; this slice does not authenticate units or infer economic formulas.

## Evidence and privacy

Every report remains `status: unverified`, `coverage: supplied_fixture_only`,
with no verified source scope/offset and `completeWithinConfiguredScope: false`.
Individual fixture comparisons may be satisfied, violated or unavailable. CLI
success means the command produced a diagnostic, not a financial all-clear.
Missing/null inputs, scale/type mismatch, malformed data, precision overflow or
work-budget exhaustion cannot become a satisfied comparison.

Reports contain only configured policy/check identities, revision, fingerprint
and finite result/error codes. Do not echo payload values, paths, source parties,
contract IDs, raw syntax errors or file names. Policy/check identifiers are
intentional report metadata; do not put secrets in their names. Configuration or
payload claims such as `complete: true` cannot confer verified evidence.

## CLI and module boundaries

- `cantongrid reconcile validate <policy.json>`: strict offline policy validation.
- `cantongrid reconcile evaluate <policy.json> <fixture.json>`: sanitized diagnostics.
- Policy code stays in a dedicated `models::reconciliation` module, reusing exact
  model evaluation primitives. CLI dispatch lives in its own command file.
- Existing model fingerprints, SQL, source plans, API behavior and migrations stay
  unchanged. Package versions advance only for unreleased development.

## Live track gate

Before querying actual models, approve a separate plan covering a coherent source
boundary and active model revisions, complete configured party scope before any
read/count, same-asset/unit/group selection, retained coverage and source pruning,
current caller/source grants and revocation before response encoding. Scope gaps
must return unavailable/incomplete, never all-clear. A user-supplied fixture cannot
supply this evidence. No live REST/GraphQL or `invariant_results` storage is part
of this branch's first slice.

## Acceptance

Exact values above 2^53, one-last-digit differences, negative/zero values, checked
addition/multiplication, Int64/Numeric overflow, mismatched scales, missing values,
ambiguous documents, canonical fingerprints, work bounds and redacted failures.
CLI tests run outside the checkout and cannot acquire source authority, mutate
files or contact services. Authorization/revocation acceptance is a live-track
gate, not a test outcome inferred from offline arithmetic.
