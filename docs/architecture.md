# Architecture

The indexer and APIs are Rust. Python under `scripts/devtools` manages local
infrastructure and verification. Applications supply selections and model
mappings; the engine has no Outcomia-specific branches.

```mermaid
flowchart LR
    L[Ledger user and current rights] --> W[Source authorization checks]
    P[Read-only PQS source] --> W
    W --> D[(Owned PostgreSQL)]
    C[Caller token and current grants] --> A[REST / GraphQL]
    D --> A
    M[Model definitions] --> T[Validation / compiler / evaluator]
    T --> D
```

## Modules

| Crate | Responsibility |
| --- | --- |
| `core` | Exact values, source identity, lifecycle, visibility and leases |
| `postgres` | Restricted connections and redacted database errors |
| `pqs` | Version-checked read-only snapshot/update paging |
| `ledger` | Current source-user and grant checks |
| `storage` | Owned migrations, state, history, checkpoints, outbox and projections |
| `runtime` | Worker, caller authentication and REST/GraphQL |
| `models` | Declarative definitions, exact evaluation and schema/client generation |
| `metadata` | Bounded offline DAR/LF 2.1 signature extraction |
| `cli` | Command dispatch |

## Source adapter

The [tested profile](verification.md#environment) uses a SELECT-only nonowner PQS
role and a separate Ledger credential with the exact configured CanReadAs set.
Source SQL is never a migration target.

| Schema041 interface | Use |
| --- | --- |
| `active(qname, offset)` | Active snapshot at a retained source-local boundary |
| `creates(qname, from, to)` / `archives(qname, from, to)` | Inclusive retained interval; resume after the acknowledged event key |
| `oldest_offset`, `latest_offset`, `pruned_offset` | Resolve explicit bounds inside the same read-only snapshot |

Pages use repeatable-read and order by offset plus source event primary key, so
events sharing an offset are not skipped. Archives require a previously observed
matching creation because schema041 omits archive witnesses. Missing rows are
not archives. History starts at the active snapshot and continues forward;
pruned/regressed bounds stop progress rather than inventing missing history.

The adapter uses the PostgreSQL driver for runtime template selection and checked
paged reads. SDK compatibility observations are in [verification](verification.md#source-observations).

## Destination and recovery

`storage init` explicitly applies checksummed migrations 001–008. Opening a store
only verifies it. Source-scoped advisory locks coordinate writers; expected
checkpoint comparisons reject stale workers. Canonical state, evidence/history,
model changes, checkpoint and outbox commit together. Exact duplicate deliveries
have no new effects. After an uncertain commit acknowledgement, reconnect and
inspect progress before retrying. Outbox sequence gaps are not ledger gaps.

The fixture reference path replays a bounded journal: 256 batches, 1,000 events
or 1 MiB per batch, 16 MiB journal text per generation. Live indexing updates SQL
rows directly and does not inherit the full-journal replay cap. Retained history
grows; automatic retention/erasure is not implemented.

Plans bind source/network/generation, templates, parties and physical endpoints/
database identity. Credential rotation preserves identity; changing a source
requires a reviewed generation. Source offsets are not global heights or counts.

## Models and queries

The registry stores immutable definitions by namespace/name/revision. Installation
compiles them into private tables and verifies real columns, keys, indexes,
ownership and grants against the plan. Identical definitions can share a physical
table; every row retains stream and contract identity. Registration and installation
do not activate processing or grant access.

Atomic `models bind` and staged rebuilds use the same source lock as ingestion.
Reads after acquiring it use READ COMMITTED to observe the preceding writer's
commit. Workers verify every bound alias and deduplicate shared physical models.
Create/archive projection updates join the canonical transaction.

For larger rebuilds, the inactive target is reserved and filled in bounded pages.
Ingestion transactionally queues changed contract IDs; catchup reconciles their
current state. Activation checks the expected old binding and empty queue before
switching metadata. Cleanup deletes unreferenced old rows in later bounded steps;
aliases retain their rows. Cancellation cleans only the inactive target.
See [models](guide/models.md) for commands, phases, limits and recovery.

Generic and model APIs use caller-scoped counts and keyset pagination in one
request snapshot. Canonical queries count/select authorized identities before
encoding page payloads. Generated model REST/GraphQL share validation and exact
native sorting. [API](guide/api.md) documents transport formats;
[security](security.md) defines authorization, final delivery checks and limits.

## Offline DAR metadata (#23)

`cantongrid metadata inspect <artifact.dar>` emits versioned JSON from a local
artifact. The isolated `cantongrid-metadata` crate reads ZIP entries without
extracting files and uses a metadata-only protobuf projection. It verifies each
original payload hash, resolves qualified signature references across packages
and preserves recursive types as references. Expressions and values are skipped;
this is not an executable LF validator or proof of deployment.

Only LF 2.1 is supported. Input, expansion, traversal and output bounds are in
[the metadata guide](guide/metadata.md). Unused interned type links are checked
once without expanding their trees; reachable signature types have separate
depth and work limits. No Ledger, database, SDK, JVM or protoc is required.
Offline schema synchronization (#24) is implemented. Reviewed additive activation (#25)
uses explicit model definitions; see [package upgrades](guide/upgrades.md).

Dependency review and approval, 2026-11-21:

| Component | Decision |
| --- | --- |
| `canton-lf` 0.3.2 | Evaluated at SDK commit `5caaebc3eab380de827196663d4bb18b90ffd6f4`; ZIP 2.x fails current RustSec policy. Not added; no scanner exceptions. |
| Official LF archive schema | Metadata field projection from that pinned SDK's vendored Digital Asset Apache-2.0 schema. Source paths/checksums are in `crates/metadata/src/wire.rs`; attribution is retained in NOTICE. |
| `zip` 9.8.1 | Approved; default features disabled, DEFLATE only. Count checked before collecting entry metadata; no disk extraction. |
| `prost` 0.14.6 | Approved standard protobuf decoder. SHA-256 is checked against original payload bytes before interpretation. |
| `serde`, `serde_json`, `sha2` | Existing dependencies reused. |

Synthetic corruption/limit tests run in CI. Independent compiler inventory
comparisons for PoC, Visibility and Outcomia are dated in [verification](verification.md).

## Selection upgrade publication

A reviewed manifest binds the old/new additive selection, checkpoint, DAR hash,
installed targets and expected previous bindings. Migration 007 retains completed
handoff evidence; 008 stores private paged jobs. Snapshot staging does not change
canonical state. Publication/cancellation gate canonical queries, discovery,
model reads and ingestion until the complete old or new prefix is available.

Each page has its own transaction and source-writer lock. Promotion changes the
selection and model aliases in the final transaction; source checkpoints and batch
history remain unchanged. Forward ingestion then reads both package versions.
Cancellation removes only unpublished baseline rows. After completion, model
rollback uses the existing rebuild path while retaining the widened selection.

## Remote transport and schema synchronization (#22–#24)

Next source slice: retain the default numeric-loopback HTTP policy and add an
explicit HTTPS policy for Ledger and health endpoints. Validate endpoint shape,
keep redirects/proxies disabled, preserve bounded timeouts/bodies, and bind the
transport and identity-provider settings into source/cursor identity. PostgreSQL
remains a separate numeric-loopback connection (an operator tunnel is possible);
this does not introduce remote PostgreSQL TLS support or live-network acceptance.

Dependency decision approved on 2026-11-21: enable the existing `reqwest` 0.13.5 `rustls`
feature in ledger/runtime for certificate-verified HTTPS. This adds its Rustls,
platform-verifier and AWS-LC dependencies; pin the resolved graph in Cargo.lock and
run the existing vulnerability/license checks before use. ISC and the trust-anchor data license CDLA-Permissive-2.0 are added to the
permissive-license allowlist for this approved TLS graph; upstream notices remain
required. This is not an advisory or license-scanner exemption. No insecure-certificate
switch, plaintext fallback, new Ledger SDK or runtime executable is proposed.
The supplied build spec §2 requires approval for new third-party hard dependencies.

Schema sync will reuse `cantongrid-metadata` and `cantongrid-models`. Developers
pin exact old package identities and explicit projections/processing rules. An
offline command resolves the corresponding new package/template, validates selected
field paths/types/scales, reports whole-signature changes and emits models, SQL,
GraphQL, TypeScript and a compatibility report. Missing/ambiguous identities,
removals and incompatible selected types must stop generation, not guess a rename
or rewrite a calculation. Installed revisions, databases, grants and source
checkpoints are untouched. The existing reviewed upgrade command remains the
separate activation step.

Remote IDP verification uses the pinned Canton 3.5.7 OpenAPI profile and the
[official JSON API definition](https://docs.digitalasset.com/build/3.5/reference/json-api/openapi.html).
User reads accept `identity-provider-id`; rights reads do not expose that query
parameter and derive their provider context through authenticated Ledger handling.
An explicit expected token issuer and returned provider must both match; no issuer
or provider is inferred from an endpoint hostname.

## Next event foundation milestone

[M0 exercise-source feasibility](design/exercise-source.md) is verified on the
pinned LocalNet. PQS tree-mode receipts match authorized Ledger Effects in four
party scopes. Partial views can disclose children without their factory exercise;
general immediate-parent proof remains unsupported. M1 should extend the existing
PQS adapter with internal exercise receipts while preserving archive ownership.
M0 adds no production event variant, destination migration or public API.
