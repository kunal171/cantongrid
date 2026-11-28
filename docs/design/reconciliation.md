# Design: reconciliation

The contract behind 0.5.0-alpha.1 and alpha.2. Usage is in the
[reconciliation guide](../guide/reconciliation.md).
- **Scope:** per-contract checks over one exact active model revision.
- **Not covered:** totals across contracts, and economic or asset inference.
- **Additions:** no new dependency, migration, source grant or ledger write.

## Policies and evaluator

- **Document:** a version-1 policy with 1–32 named expressions and 1–16 checks.
  - **Expressions:** reuse the model expression compiler and checked arithmetic,
    with required Int64 or Numeric outputs only.
  - **Checks:** `eq`, `ne`, `lt`, `lte`, `gt` and `gte`, at the same scale.
  - **Not allowed:** rounding, defaults, implicit zeros, approximate tests, SQL or
    plugins.
- **Limits and parsing:** the existing 64 KiB, depth-16 and 256-node budgets. JSON
  is strict.
- **Fingerprint:** canonical, so ordering and whitespace don't change it.
- **Code layout:** `models::reconciliation` holds the logic, and the CLI dispatch
  lives in its own file. Existing fingerprints, SQL and APIs are unchanged.
- **Units:** asset and unit correctness is the developer's assumption. Matching
  scales don't prove it.

## Evidence and privacy

- **Fixture reports:**
  - **Status:** they're always `unverified` with `supplied_fixture_only`.
  - **Failures:** a missing input, a mismatch, an overflow or an exhausted budget
    can never become `satisfied`.
  - **Claims in config:** a claim such as `complete: true` in a config or payload
    confers nothing.
- **What reports include:** only configured identities, the revision, the
  fingerprint and finite codes. Values, paths, parties, contract IDs, raw errors
  and file names never appear.
- **Preflight:**
  - **Selection:** checks one explicitly named model.
  - **Fields:** each policy field must map to exactly one projected field of the
    same type and scale.
  - **Rejected:** nested paths and checks made only of constants.
  - **Identity:** it binds the namespace, model and policy into a domain-separated
    identity.
  - **What it isn't:** a schema check only, with no lookup and no totals query.

## Indexed checks (`Store::read_reconciliation`)

1. **Authorization first:**
   - **Inputs:** the trusted backend supplies authenticated `CallerGrants` and a
     current `SourceLease`.
   - **Scope:** the exact source scope and the caller's **complete** party scope
     are required before any data or count query.
   - **Why:** this way hidden inputs can't change whether a narrow request
     succeeds.
2. **One snapshot:** a single read-only repeatable-read transaction covers the
   live plan and prefix, the exact active revision and fingerprint, and a fresh
   observation.
   - **Required:** a completed schema041 boundary and bounded retained coverage.
   - **Unavailable otherwise:** snapshots, partial intervals, pruned evidence and
     stale sources.
3. **Bounds:**
   - **Size:** 128 canonical contracts, counted *before* the model filter, and
     8 MiB, under the statement timeout.
   - **Projection rows:** every expected projection row and the native row count
     are verified, which catches missing, filtered, archived or extra
     contributions.
   - **Over budget:** the read fails as unavailable.
4. **Evaluation:**
   - **Inputs:** canonical payloads are re-evaluated with the installed model.
   - **Comparison:** native records are compared without floats.
   - **Report:** only identities and summary counts.
   - **Empty selection:** unavailable, never a vacuous success.
5. **Recheck:**
   - **Storage:** the lease is checked before and after the read.
   - **API:** the API layer re-authenticates again before encoding.

A satisfied report doesn't certify balances or financial health.

## API (alpha.2)

- **Manifest:** an optional reconciliation manifest, resolved relative to
  `runtime.json`.
  - **Entries:** 1–16, each with a unique name and pins on the namespace, the model
    revision and fingerprint, and the policy file and fingerprint.
  - **Policy files:** they resolve relative to the manifest.
  - **Mismatches:** duplicate names and pin mismatches fail closed.
  - **Registry:** there's no SQL registry.
- **Service:**
  - **Module:** `api/reconciliation` owns the REST endpoints
    (`GET /v1/reconciliation` and `POST /v1/reconciliation/{name}/query` with `{}`).
  - **GraphQL:** a dedicated `/reconciliation/graphql`.
  - **Inputs:** requests never supply formulas, schemas, paths or grants.
- **Order of checks:**
  - **Before lookup:** authentication, then a source refresh, then the full scope.
  - **Before returning:** the manifest is reloaded and the identities, source
    authority and caller grants are compared again.
  - **GraphQL:** any failure clears all of `data`, and an operation has at most 2
    roots.
- **Status codes:** 401, then 403 (before lookup), then 400, then 404 (only after
  the full-scope check), then 503.
  - **Catalog validation:** conservative. One inactive pin makes the endpoint
    unavailable.

## Acceptance

- **Offline:**
  - **Numbers:** values above 2^53, last-digit differences, negatives and zeros,
    checked arithmetic, overflow, scale mismatches and missing values.
  - **Documents and limits:** ambiguous documents, fingerprints, budgets and
    redaction.
  - **CLI isolation:** CLI tests can't reach services or acquire source authority.
- **Indexed:**
  - **Consistency:** coherent reads during ingestion and promotion.
  - **Selection and drift:** selection, projection drift, and empty or
    over-budget sets.
  - **Coverage:** incomplete, pruned or stale coverage.
  - **Authorization:** narrow callers, including denial that doesn't depend on the
    data, and expired, revoked or mismatched leases.
- **API:** REST and GraphQL agree, narrow-scope denial, pin and config changes,
  promotion, revocation and multi-root failures.
- **LocalNet (read-only):**
  - **Method:** compare PQS and Ledger inventories at one offset, then check
    `priceTick * 10000 == priceTickBps` and `quantityLot > 0` through both APIs.
  - **What it proves:** the plumbing, not solvency.

The v0.5.0 cross-contract plan is a separate proposal.
