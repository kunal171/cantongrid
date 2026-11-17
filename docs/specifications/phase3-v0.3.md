> Imported from the user-supplied v0.3 handoff on 2026-11-17. Its implementation
> baseline predates PR #3. Current implementation and test evidence are in
> [verification](../verification.md) and the [source contract](../phase3-source-contract.md).
> Phase 3 remains the accepted scope; later phases retain separate acceptance.

# CantonGrid — Technical Build Specification and Codex Handoff

**Version:** 0.3 · 17 November 2026  
**Repository:** https://github.com/kunal171/cantongrid  
**Status:** Phase 2 complete at fixture scope; **Phase 3 next**  
**Runtime:** Rust (Python only for development and verification helpers)  
**First real consumer:** [Outcomia Canton](https://github.com/kunal171/outcomia-canton)  
**License goal:** Apache-2.0, pending dependency review and actual publication

> **Codex: continue the existing repository. Do not scaffold a replacement or execute all roadmap phases. Implement Phase 3 only.** Read `AGENTS.md`, `docs/decisions.md`, `docs/roadmap.md`, `docs/research-decisions.md`, `docs/security.md`, `docs/verification.md`, and `docs/competitive-landscape.md` before modifying code. Current user instructions and accepted decisions supersede historical build prompts.

## 1. Product, scope and sequence

CantonGrid is an **open-source, authorization-aware, developer-configurable data infrastructure framework for the Canton Network**. Developers choose data they are authorized to read, define what/how to process, and obtain persistent PostgreSQL models, authenticated APIs, and eventually resumable data streams.

Long-term, the framework may support independent data **tracks**: Daml contracts, public Scan/network, assets, governance, and authorized operator telemetry. **The accepted first release is contracts-only** (`D-001`, `D-006`). Build seams for additional sources but do not implement Scan or other tracks in this Codex task. Do not claim a globally accessible private ledger, global history, universal ordering, transaction submission or a replacement for official PQS.

**Outcomia is the mandatory first real integration**, never an engine-specific conditional. Also prove a second unrelated Daml configuration using the same core, storage and API path. Domain-level trade/order math is not part of the generic ingestion engine.

| Phase | Deliverable | Evidence today |
| --- | --- | --- |
| 0 | Product/specification decisions | Documented |
| 1 | LocalNet, PQS and package compatibility | Partially verified; some live gates open |
| 2 | Rust configuration, lifecycle fixtures and privacy primitives | **Complete at fixture scope** |
| **3 — Codex next** | **Verified authorized PQS reads → owned PostgreSQL + recovery → authenticated generic REST/GraphQL lifecycle queries** | **Not implemented** |
| 4 | Declarative typed derived models, generated migrations and query fields | Planned |
| 5 | Durable authenticated SSE, operational tooling, first release | Planned |
| Future | Scan, assets, governance, operator/custom source adapters | Deferred |

### Already built — preserve this baseline

- Rust 1.100 / edition 2024 workspace in `crates/core` and `crates/cli` with `init`, `validate` and `fixture run` working. The planned `index`, `serve` and `dev` workflows **are not implemented**.
- Strict JSON configuration, source/network/scope identities, declared capabilities, exact tagged Numeric/Int64, create/archive lifecycle state, replay/idempotency, invalid-batch rollback and scoped authorization decisions are fixture-tested **in memory**. Fixture caller grants are simulated, not live API authentication. Previous verification: **29 Rust and 6 Python tests**; rerun rather than assume the count.
- Local development helpers examined Canton 3.5.7 with Scribe/PQS 3.5.4 (`schema 041`) and scoped SQL/visibility. This is **not** a live Rust PQS connector test. Upstream releases and schemas may differ.
- The existing Outcomia DAR was inspected without upload; it has **not** been indexed by the Rust runtime. An unrelated visibility example has development evidence and uses the same fixture engine.
- No live Rust connector, CantonGrid-owned PostgreSQL canonical store, authenticated live REST/GraphQL service, generated business projection or durable event stream is implemented yet.

Read `docs/localnet-evidence.md`, `docs/pqs-compatibility.md`, `docs/outcomia-compatibility.md`, `docs/visibility.md` and `docs/verification.md` for precise evidence and limitations.

## 2. Competitor overlap and genuinely testable differentiation

Review the source-linked stages in [`docs/competitive-landscape.md`](../competitive-landscape.md) before discussing grants or claiming novelty:

| Existing product/proposal | Verified situation (17 November 2026) | Consequence |
| --- | --- | --- |
| Official PQS | Released participant-visible contract/query storage; we locally evaluated an older Scribe schema | Use authorized documented reads, **not** a new participant-indexing implementation |
| Canton Index / InfraDAO | Runnable `cinder` and indexer-server v0.10.0, PQS-to-Postgres projections, cursors; source not fully public yet; milestone submissions under review | Rust, PQS filters, PostgreSQL and recovery **are not unique** |
| Zpoken | Open-source seeded-data GraphQL/JWT PoC; grant proposal open | Public/private indexing, party auth and future DAR generation are already proposed; live PQS ingest not shown by its PoC |
| Noves Data App | Self-hosted deployable packaged product with APIs, reports, alerts, streams; public repo largely distribution assets | There is existing product overlap; do not call Noves fully open source |
| DSRV Canton Lens | Open-source direct Ledger API explorer without durable history index | Explorer/read API alone is not our novelty |
| Nodejumper Canton Rust SDK | Public Rust SDK with typed PQS access | Check SDK compatibility before authoring another general-purpose client |

**Our differentiation must be proved, not asserted:**

1. **Developer-declared deterministic models (Phase 4):** supported typed definitions specify selectors, lifecycle dependencies, exact operations and derived output. The engine generates SQL schemas/migrations and query fields without handwritten application-specific Rust/SQL/resolvers. Prove Outcomia plus unrelated app, replay equivalence, archive retractions and model-version upgrades. Canton Index proposes stateful processing later, so compare specific delivered capabilities, not conceptual ownership.
2. **Transitive privacy (Phase 3 foundations, Phase 4 completion):** source access and end-user API rights differ. A derived output depending on A-private and B-private inputs cannot leak to an A-only or B-only caller through rows, counts, joins, errors, caches, GraphQL, SSE, or cursors. Validate source/caller grant intersection and authorization epoch; fail closed when revoked or unknown.
3. **Machine-readable provenance and completeness (Phase 3):** each query identifies source, permitted party scope, source-local watermark, freshness and whether history is complete over the available interval, partial, gapped or unknown. Never represent a participant-local offset or limited view as an entire global ledger. Similar concepts exist in other proposals: novelty is **tested enforcement**, not the phrase “data provenance.”
4. **Independent future tracks (deferred):** a later public Scan-only source must not inherit PQS visibility and its cursor must remain source-local. Keep minimal separation now, do not build Scan in Phase 3.
5. **Interoperable components:** PQS owns its schema; evaluate Nodejumper's SDK and existing source protocols, logging a decision before duplication. No new third-party hard dependency without user approval.

**Prohibited claims without new evidence:** “first Canton indexer,” “only privacy-safe indexer,” “faster,” “production-ready,” “exactly once globally,” “complete Canton data,” “all packages with zero configuration,” or “grants approved.” Product and grant stages may change; refresh official sources when preparing public proposals.

## 3. Codex assignment — implement Phase 3 only

**Objective:** deliver a reproducible **authorized, read-only LocalNet/PQS → CantonGrid-owned PostgreSQL → authenticated generic REST/GraphQL** pipeline with crash-safe lifecycle replay and explicit coverage/provenance. Do not implement generated typed business models (Phase 4) or SSE (Phase 5).

### Step 0 — inspect before edits

1. Read `AGENTS.md` and accepted design/security/verification docs. Check repository status and preserve existing changes. Inventory current crates and commands; run existing formatter/tests/fixtures; capture actual results.
2. Verify exact Scribe/PQS version and functions, offset semantics, source/filter/party permissions, snapshot boundary, pruning behavior, and relevant official SDK documentation. Do not invent an endpoint, SQL column, function, transaction ID or data type.
3. Evaluate the Nodejumper Rust PQS SDK **against the actually deployed Scribe 3.5.4/schema041**. Reuse where compatible; otherwise record a concrete reason for a thin, read-only adapter.
4. Record resolved interface evidence and blockers in `docs/research-decisions.md` and/or `docs/phase3-source-contract.md` before coding an unverified live adapter.

### Step 1 — prove source permissions and snapshot → update handoff

- Use a **read-only, explicitly scoped** source identity and locally configured secrets. Never log/commit credentials. The connector's source privileges do **not** become API-user privileges.
- Determine how the verified version exposes active state, creates, archives, exercises **only if available**, updates/offsets, origin identifiers, party visibility, history retention and recovery after disconnect.
- Establish an **observably no-gap, source-local handoff** from an initial active snapshot to subsequent updates. Capture a consistency boundary and dedup rules based on real source semantics.
- If a lossless handoff cannot be demonstrated, **do not claim reliable live indexing**. Document the exact gap and stop at a safe, clearly labeled partial-history mode or request a decision; no invented history.

### Step 2 — implement minimal read-only Rust connector

- Fit the adapter behind existing source/capability interfaces; preserve fixture behavior. Do not mutate upstream PQS/Scribe tables, submit ledger transactions, provision parties or upload/vet DARs.
- Map verified create/archive lifecycle and package/template/contract IDs to existing canonical types. Capture source, network, party-scope identity, upstream event identity, source-local offset, visibility, payload/schema version and provenance.
- Preserve exact Daml Numeric/Int64 using decimal/string/integer types, never `f32` or `f64`; reject or transparently mark unsupported values instead of silently coercing.
- Bound ingestion batches/memory; retry transient failures without dropping scope filters, inferring archives or advancing past uncommitted data.

### Step 3 — PostgreSQL canonical storage, atomic checkpoints and recovery

- Add versioned migrations for **only CantonGrid-owned tables**: source/scope identities, canonical lifecycle events, active contract state, per-source checkpoints, schema/processing version, coverage metadata and a durable outbox reserved for later streams. **Never migrate PQS's own database.**
- Define idempotency keys from verified upstream identity plus network/source/scope or subscription generation as required. Do not presume globally ordered Canton block heights or reuse offsets across sources.
- Commit lifecycle event, active-state transition, outbox record and checkpoint advancement **in the same destination DB transaction**. On failed transactions, checkpoints stay unchanged. Replay/duplicate deliveries produce the same final state.
- Make snapshot coverage, known gaps/retention and earliest-observed boundary explicit (`complete`, `partial_history`, `gap`, `unknown` or equivalently documented statuses). Do not manufacture inaccessible history or erase canonical data casually to recover.
- Handle source outage/reconnect, destination outage/restart, permission/filter generation changes and schema version changes with safe re-snapshot or fail-closed status.

### Step 4 — authenticated generic query serving

- Implement planned Rust `index` and `serve` CLI paths (or a documented equivalent consistent with `docs/cli.md`); preserve `init`, `validate` and `fixture run`.
- Expose the canonical **active-contract** and **supported lifecycle/history** queries with authenticated REST **and** generic GraphQL. Typed generated business-domain GraphQL remains Phase 4.
- The server validates caller identity and configured **party-subset grants**; never trust a query-supplied party ID alone. Apply source∩caller grants to every row, count, selection, page cursor, cache key, error and GraphQL resolver. Include lease/epoch checks and fail-closed revocation behavior.
- Return authorized source/scope provenance, coverage status, freshness and source-local watermarks. Mark unavailable/stale/partial states explicitly, never claim global completeness.
- Bind page cursors to source/scope/grant or epoch; reject cross-principal, stale, expired or widened cursors. Bind development listeners to loopback unless explicitly configured otherwise.

### Step 5 — Outcomia and genericity demonstrations

1. Resolve Outcomia's actual package/template identifiers against verified metadata and an **authorized LocalNet** source. Do not upload Outcomia's DAR, create parties, execute contracts, or submit trades without separate authorization. When no Outcomia data exists, report that its live indexing proof is blocked; do not generate fake evidence.
2. Index an unrelated Daml example through the **same** connector, configuration, schema and APIs. No Outcomia-specific production branches or hardcoded schemas.
3. Exercise A, B and AB API identities: A-only sees only A-granted records, B-only only B-granted, AB only their entitled union. Revoked/expired grants fail closed. Test active/history/counts/pagination on REST and GraphQL.
4. Verify create→archive, exact Numeric/Int64, duplicate updates, interrupted DB commit/retry, process restart, source outage, filter changes, and partial-history disclosure.
5. If source access or permissions are unavailable, provide **clearly labeled fixture/owned-Postgres tests** and list live integration work as incomplete instead of claiming Phase 3 passed.

### Step 6 — update tests, docs and commits

- Add unit tests for conversion, identities, grant logic and offset boundaries; destination DB integration tests for atomic state+checkpoint+outbox; authorized LocalNet integration tests when permitted.
- Keep CI tests credential-free. Update `docs/architecture.md`, `docs/security.md`, `docs/verification.md`, `docs/cli.md`, and relevant compatibility docs only for code actually delivered.
- Make small reviewable commits (source contract, connector, own DB schema/recovery, auth/API, demo/verification). No monolithic full-repository rewrite.

## 4. Phase 3 acceptance matrix

| Requirement | Mandatory evidence | Condition |
| --- | --- | --- |
| Authorized live source | Exact versions and repeatable nonsecret PQS transcript | Real restricted read, or **blocked/not complete** |
| Snapshot→updates | Controlled boundary/cursor test | No create/archive event missed at the handoff |
| Crash-safe storage | DB migrations and interrupted-transaction tests | State, event, outbox and cursor advance atomically |
| Idempotency | Duplicate replay, reconnect and restart | Identical final state, no phantom checkpoint |
| Privacy | A/B/AB, revocation, pagination, counts, both API transports | No private value, aggregate or metadata leak |
| Genericity | Outcomia and unrelated Daml configuration | Same production engine; actual Outcomia data clearly distinguished from fixture |
| Exactness | Decimal and Int64 round trips | No precision loss, silent coercion or unsupported type masking |
| Coverage | Partial/pruned/unknown source cases | No false global completeness assertion |
| Regression | Existing CLI/fixture tests | Previously verified behavior still works |

**Baseline commands to run** (use available compiler/dependencies and record actual results):

```bash
cargo +1.100.0 fmt --all -- --check
cargo +1.100.0 clippy --workspace --all-targets --locked -- -D warnings
cargo +1.100.0 test --workspace --locked
python3 -m unittest discover -s scripts/tests -v
```

Provide additional Postgres and authorized LocalNet integration commands once implemented. Explicitly label test reports **fixture-tested**, **owned-Postgres-tested**, **LocalNet-tested**, **DevNet-tested**, or **production-tested**. Unavailable infrastructure must be recorded as a blocker, not a pass.

## 5. Security and correctness invariants (all phases)

1. **Permission correctness is end-to-end:** source reader grants never imply API caller grants; use the narrowest dependency authorization for derived outputs and query results.
2. **No unapproved external writes:** no DAR uploads, party provisioning, ledger commands, remote deployments, upstream DB modification or third-party proposal changes. Do not alter either Outcomia repository.
3. **Source-local provenance:** keep source, network, scope, event identity, generation, position, freshness and coverage together; no fictitious chain-wide ordering.
4. **Exact data:** decimals/large integers round-trip unchanged. Unknown type/history/permission states fail explicitly and safely.
5. **Deterministic processing:** validated, versioned definitions only. Future AI authoring assistance must never control runtime authorization or financial accounting.
6. **Revocation:** previously materialized data, counts, cursor reuse, caches and later streams must not leak after grant removal; fail closed on uncertainty.
7. **Safe recovery:** transactionally couple canonical updates, outbox and checkpoints; never promise universal exactly-once semantics based only on internal idempotency.

## 6. Later milestones — explicitly NOT part of this Codex task

**Phase 4 — differentiation:** introduce bounded typed selection and transform definitions, deterministic lifecycle-aware calculations, versioned generated PostgreSQL models/migrations and typed GraphQL fields. Test Outcomia plus unrelated Daml app **without any engine source change**, snapshot/replay equivalence, archive retractions, schema upgrade/rollback and transitive authorization of aggregates/joins. Compare what ships against Canton Index, not only proposed roadmaps.

**Phase 5 — streams/release:** consume the committed outbox through authenticated resumable SSE, scoped cursors, retention/gap detection, reconnect/revocation tests and operational diagnostics. Measure performance and publish licensing only when ready.

**Future tracks — separate acceptance:** public Scan, assets, governance and operator telemetry; independent connector permissions and source-local positions. These are not a workaround to accessing restricted private data.

## 7. Copy/paste Codex kickoff prompt

> Work in the existing `https://github.com/kunal171/cantongrid` repo. Read `AGENTS.md`, `CantonGrid_Technical_Build_Spec.md` v0.3, and the decisions, roadmap, source-research, security, verification and competitive-landscape docs first. Preserve the already completed Phase 2 fixture implementation. **Implement Phase 3 only:** a verified authorized LocalNet/PQS **read-only Rust connector**, CantonGrid-owned **PostgreSQL canonical lifecycle state/outbox/atomic checkpoints**, and **authenticated generic REST/GraphQL active-contract and lifecycle queries**. Outcomia is the first real integration; prove a second unrelated Daml configuration without app-specific engine branches. Preserve exact Numeric/Int64 values, honest source-local coverage, A/B/AB party isolation, revocation safety and restart/replay recovery. Verify actual PQS APIs and evaluate the Nodejumper SDK before coding; do not invent interfaces, write to upstream PQS, or claim a complete Canton ledger. No Scan track, generated business model engine or SSE in this task. Do not modify Outcomia repositories, upload DARs, provision parties, submit Canton transactions or deploy outside authorized scope. Commit in small steps, run the recorded tests, and report exact verified environment, incomplete gates, and commit IDs. **Ask if an unapproved operation or missing permission blocks work.**

## 8. Authoritative project references

- [Accepted decisions](../decisions.md) · [Roadmap](../roadmap.md) · [Working rules](../../AGENTS.md)
- [Research gates](../research-decisions.md) · [PQS observations](../pqs-compatibility.md) · [Outcomia package evidence](../outcomia-compatibility.md)
- [Security](../security.md) · [Architecture](../architecture.md) · [Processing model](../processing-model.md)
- [Competitive landscape](../competitive-landscape.md) · [Verification](../verification.md) · [CLI](../cli.md)

*This v0.3 spec supersedes the original v0.2 multi-track build prompt. Its Scan MVP and “implementation not started” statements are historical and must not guide current Codex work.*
