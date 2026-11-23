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

## Model preflight: next offline slice

Add `reconcile preflight <policy.json> <catalog.json> <namespace> <model>`.
Validate one explicitly selected model, not the first catalog entry. Every policy
field reference must name exactly one projected field with the same required
Int64/Numeric type and scale. Reject nested/raw-payload paths, missing fields and
literal-only policies. Constants and checked arithmetic remain supported.

The result binds the namespace, full immutable model fingerprint/revision and
policy fingerprint/revision into a domain-separated identity. It confirms schema
compatibility only: no registration, active-binding lookup, SQL, source access or
coverage claim. Even a filtered or aggregate-enabled model is interpreted only
as its projected per-contract rows; preflight does not create a totals query.

## Live track gate

Before querying actual models, approve a separate plan covering a coherent source
boundary and active model revisions, complete configured party scope before any
read/count, same-asset/unit/group selection, retained coverage and source pruning,
current caller/source grants and revocation before response encoding. Keep global
source offsets operator-private or represent them with opaque evidence handles. Scope gaps
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

## Proposed indexed-contract slice (approval pending)

Start with **per-contract checks over one exact active model revision**, not
cross-contract totals or economic asset inference. The existing model template,
filter and projections define the input selection. The operator must review the
formula and units; matching Numeric scales does not establish asset agreement.
Cross-model/group reconciliation needs its own asset/unit selection design.

1. Storage-only read method, with independently authenticated `CallerGrants` and
   current `SourceLease` supplied by the trusted backend. Require the exact plan
   source scope and the caller's complete configured party scope before any data
   or count query. Never expose narrow-scope partial comparisons.
2. Use one read-only repeatable-read transaction. Verify the live plan/prefix,
   exact active model revision/fingerprint and fresh source observation. Require
   a completed schema041 boundary and bounded retained journal/selection coverage;
   snapshots, partial intervals, missing/pruned evidence and stale sources fail
   unavailable. These are indexed-as-of checks, not a claim of current ledger head.
3. Bound work to 128 selected canonical contracts and 8 MiB of input/output, with
   the existing statement timeout. Check the canonical model selection and
   projection consistency together; missing/drifted projection rows cannot vanish
   from the checks. Excess work fails unavailable, without a truncated success.
4. Evaluate the same compiled exact policy per selected contract. Return only
   configured identities and finite summary counts/outcomes, with no payloads,
   contract IDs, witnesses, group keys or global source offsets. An empty selection
   is explicitly unavailable, never vacuous success. Even satisfied scoped checks
   do not certify balances, economic correctness or overall financial health.
5. Recheck source lease and complete caller scope before returning. A future HTTP
   adapter must independently reauthenticate/revalidate current source and caller
   grants immediately before encoding; storage lease checks alone are not an API
   revocation guarantee. No public route is added by this slice.

Acceptance: coherent reads during concurrent ingestion and model promotion;
create/archive/filter selection; exact arithmetic/overflow; projection drift;
empty and over-budget sets; incomplete/pruned/stale coverage; narrow callers
including data-independent denial; expired/revoked/mismatched source leases; and
redaction. Use isolated owned-PostgreSQL fixtures first. LocalNet/remote claims
require separately recorded source evidence. No new dependency, migration,
persisted result registry, ledger write, network credential change or publication.
