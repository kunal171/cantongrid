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
| First source | Scribe 3.5.4/schema 041 is verified for scoped LocalNet active/create/archive queries. A fixture SQL reader and version-specific authorization/history limits are recorded. Production grants and the Rust adapter remain open. | Source-specific code. |
| Snapshot handoff | Stable boundary, pagination, concurrent changes and resume behavior; demonstrate no lost creates/archives. | Bootstrap/checkpoint implementation. |
| Event identity and ordering | Fixture persistence verifies source-local event/contract identity and opaque position replay. Live dedup/order/time semantics remain open. | Canonical persistence. |
| Private visibility | Owned A/B/AB fresh-request matrix, exact fixture witnesses and revocation verified. Derived joins, full historical visibility and application caller isolation remain open. | Live private data copying. |
| Revocation and retention | Self-rights revalidation verified; cached B-only data remains after source revocation and readiness loss. Rust lease/invalidation decision primitives are fixture-tested; live grant retrieval/renewal, historical access, erasure and rebuild policy remain open. | Live private data copying. |
| History coverage | Pruning, unavailable ranges, filters, reassignments and other lifecycle limitations. | History completeness claims. |
| Daml schema discovery | Verified package metadata access and type representation, including upgrades and template identity. | Typed schema generation. |
| Exact values | Numeric 10 and maximum Int64 string round trips, null optional and Ready enum verified through Ledger API and PQS. Other scales, variants, timestamps and generated mappings remain open. | Typed storage/operators. |
| Outcomia mappings | Existing DAR validated without upload and initial source templates reviewed. Typed metadata extraction and authorized lifecycle/model demonstrations remain open. | Outcomia demo. |
| Genericity fixture | Unrelated Visibility:Record package built/deployed and queried without Outcomia code. Both synthetic examples pass the same Rust configuration/lifecycle runner. Declarative derived models and real source ingestion remain open. | Processing interface acceptance. |
| API identity | Token issuer/format, grant mapping, expiry, rotation and administrator roles. | Authenticated serving. |
| Model language | Strict fixture configuration v1 and capability errors are implemented. Derived-model grammar, keys and expression semantics remain open. | Public configuration contract. |
| Database ownership | Fixture SQL reader can query without table/schema writes or owner escalation. Owned local destination/schema, restricted role, transaction writer exclusion and checksum-guarded initialization are tested. Production transport/grants, incremental state loading and migration/rebuild evolution remain open. | Persistence implementation. |
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
