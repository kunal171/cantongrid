# Authenticated reconciliation API slice

Implemented in alpha.2 source; owned-PostgreSQL/mock-source controls pass.
See [verification](../verification.md#authenticated-reconciliation-api--2026-11-23).

- Runtime configuration gains an optional reconciliation manifest path, resolved relative to runtime.json. Omission preserves existing behavior; ingestion does not load policy contents.
- The bounded version-1 manifest holds 1–16 entries, each with a unique configured name and pinning namespace, model name/revision/fingerprint, policy file and policy fingerprint. Policy files resolve relative to the manifest and use the existing strict 64 KiB compiler. Duplicate names and pin mismatches fail closed. No SQL policy registry/migration.
- New module api/reconciliation owns services, REST and GraphQL. GET /v1/reconciliation lists sanitized configured identities; POST /v1/reconciliation/{name}/query accepts exactly {} and executes one server-configured policy. Requests never supply party grants, formulas, schemas or paths.
- A dedicated /reconciliation/graphql endpoint provides typed reconciliationPolicies and reconciliation(name: String!) fields over the same service. Keep report scalars exact strings. No JSON formula input or new model-generated field.
- Authenticate before processing input. Refresh current source authority and require exact source plus full caller scope before policy discovery or any model read/count. Use bounded blocking admission and Store::read_reconciliation.
- Before returning, reload manifest/config, compare immutable policy/catalog/source identities and active model definitions, refresh current source authority and caller grants. Guard serialization on both sides. Discard the whole GraphQL data object on any resolver or final-guard failure; at most two evaluation roots per operation.
- REST responses: 401 bad caller identity; 403 incomplete caller scope before lookup; 400 unsupported body/input; 404 unknown configured policy only after full-scope authorization; 503 invalid config, inactive pin, source failure or unavailable indexed evidence. Arithmetic unavailable is a successful diagnostic, not a financial all-clear.
- Test with owned PostgreSQL and mock source authority: exact REST/GraphQL equality, narrow-scope existing/unknown policy denial, malformed/unknown fields and no arbitrary policy execution, pin/config changes, active revision promotion, source and caller revocation, stale/incomplete evidence, overflow, multi-root failure and redaction. No ledger writes, dependency additions or source grant changes.
- Record LocalNet evidence separately from mock controls; remote acceptance and public publication remain later work.

Implemented in alpha.2 source. Catalog-wide pin validation is conservative: any configured inactive/incompatible entry makes this endpoint unavailable. The final synchronous delivery guard retains the existing 401 failure behavior. Evidence is recorded in docs/verification.md.


## Read-only LocalNet acceptance

Use the existing PoC contracts and verified source rights; submit no Daml command,
DAR upload, party allocation or grant update. Reuse the operations runner's
isolated destination stream and source inventory rather than adding another
infrastructure setup script. Register/bind the existing PoC model only in a new
owned namespace. Keep original PoC files and bindings intact.

At a completed PQS boundary, compare its active inventory with an authorized
Ledger snapshot at that same offset. Use exact integers for Numeric coefficients
and Int64 values. Check `priceTick * 10000 == priceTickBps` and `quantityLot > 0`
through REST and the typed GraphQL endpoint, including restart, narrow-scope
existing/unknown-name denial and isolated application-grant revocation. This
validates arithmetic/data plumbing, not collateral, trading or economic solvency.
Cap the reconciliation inventory at 128 and keep private source data/config under
`.local`; publish only counts, booleans, versions and source-code identity.
