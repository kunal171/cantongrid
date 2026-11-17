# Research and unresolved decisions

Status: LocalNet startup and baseline authenticated Ledger API reads have been
[verified locally](localnet-evidence.md). Scoped [PQS SQL compatibility](pqs-compatibility.md) and one
[Outcomia DAR validation](outcomia-compatibility.md) have also passed. Remaining
connector, schema and authorization guarantees below are still open. The original supplied brief is product
input, not compatibility evidence.

Before implementing a live adapter, consult current official Canton/Daml/PQS
primary documentation and the actual installed environment. Record exact versions,
links, observed behavior, reproduction steps and limitations alongside the result.
Do not infer current endpoints, SQL functions or payload shapes from this plan.

| Question | Required evidence / decision | Must resolve before |
| --- | --- | --- |
| LocalNet availability | Bootstrap and baseline reads verified; installed versions, endpoints and limits recorded in the LocalNet evidence. Application contract scope remains unverified. | Live connector implementation. |
| First source | Scribe 3.5.4/schema 041 is verified for scoped LocalNet active/create/archive queries. Rust adapter design, production role and broader authorization/history guarantees remain open. | Source-specific code. |
| Snapshot handoff | Stable boundary, pagination, concurrent changes and resume behavior; demonstrate no lost creates/archives. | Bootstrap/checkpoint implementation. |
| Event identity and ordering | Stable dedup keys, source positions, lifecycle kinds and time semantics. | Canonical persistence. |
| Private visibility | Exact available authorization evidence and its interpretation; source grant changes and visibility loss. | Live private data copying. |
| Revocation and retention | Revalidation lease, fail-closed behavior, historical access, erasure and rebuild implications. | Live private data copying. |
| History coverage | Pruning, unavailable ranges, filters, reassignments and other lifecycle limitations. | History completeness claims. |
| Daml schema discovery | Verified package metadata access and type representation, including upgrades and template identity. | Typed schema generation. |
| Exact values | Numeric scale/precision, Int64, optionals, variants, timestamps and transport serialization. | Typed storage/operators. |
| Outcomia mappings | Existing DAR validated without upload and initial source templates reviewed. Typed metadata extraction and authorized lifecycle/model demonstrations remain open. | Outcomia demo. |
| Genericity fixture | Unrelated template set and representative lifecycle/derived model. | Processing interface acceptance. |
| API identity | Token issuer/format, grant mapping, expiry, rotation and administrator roles. | Authenticated serving. |
| Model language | Bounded grammar, keys, expression semantics, capability errors and diagnostics. | Public configuration contract. |
| Database ownership | Namespace, roles, concurrency fencing, transaction/rebuild/migration strategy. | Persistence implementation. |
| Stream contract | Cursor protection, scope binding, retention, gaps and replay authorization. | Durable stream serving. |
| Distribution | Rust/dependency versions, license compatibility and packaging. | Executable release/publication. |

Every resolved item should state whether evidence is documentation-only,
fixture-tested, LocalNet-tested or devnet-tested. Preserve unresolved limitations
as capability restrictions. A source unable to provide trustworthy historical
visibility must not be advertised as complete authorized lifecycle history.

## Deliberately deferred choices

No commitment yet to two production connectors, GraphQL subscription transport,
a hosted control plane, arbitrary processing plugins or public Scan ingestion.
The conceptual module boundaries can become crates or internal modules as the
first executable slice reveals dependency needs. Choose dependency versions and
concrete source requests during compatibility research, not from speculation.
