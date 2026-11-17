# Authorization and private data

Status: required design properties. Concrete upstream visibility semantics and
credential validation must be verified before live ingestion.

## Two independent boundaries

The source authorizes the operator to ingest a bounded set of contract data.
CantonGrid separately authorizes each API principal to read permitted subsets.
A valid API token must never grant access beyond the ingestion scope. Filters,
party strings and client-supplied identifiers cannot grant permission.

The first deployment serves one organization with multiple scoped principals.
Token format and issuer integration remain open. Validate identity, expiry and
grants on every access path. Administrative source/health details need separate
permissions and redaction; application users must not see private operational
metadata merely because they can query a model.

## Visibility and derived data

Store verified visibility evidence and provenance with each input. Signatories,
observers, witnesses and requesting parties are not assumed interchangeable.
Use the verified source's visibility semantics rather than inventing a rule.

A derived row requires permission to all contributing inputs. For example, a join
of an A-only record and a B-only record can be visible to an AB-authorized caller,
but not to either A-only or B-only callers. Taking the union of input party labels
and permitting any matching party would leak data. Aggregates need equivalent
dependency enforcement or separate materialization for an explicit allowed scope.

Authorization applies before filtering, pagination, counting, relationship
resolution, aggregation and stream delivery. Errors, existence checks, schema
metadata, caches and cursor validation must not reveal inaccessible records.
Generated API configuration cannot disable these requirements.

## Scope changes and failure behavior

Bind snapshots, checkpoints, rebuilds, caches and cursors to source identity,
selection and authorization epoch. Internal scope fingerprints must not reveal
party identifiers. Validate requested party subsets against current grants.

Local revocation denies new access immediately, invalidates affected cursors and
caches, and terminates or reauthorizes streams. Source authorization failures fail
closed. Define a bounded source-authorization revalidation lease before live
service: unknown or expired authorization must not continue serving private data.
External revocation cannot be claimed instantaneous while disconnected.

Historical access, deletion, retention and removal of materialized private data
need explicit operator policy before live copying. Archival of a contract is not
itself a request to erase authorized history; pruning is not proof of archival.
Retained data does not automatically imply permission to serve it indefinitely.

## Durable streams

Resume cursors are opaque, integrity-protected and bound to principal/grant
context, source, model/schema version and authorization epoch. Recheck access on
every delivery and reconnect. Reject cursor reuse under another scope. Missing
retention produces an explicit resync requirement, never silent event loss.
Durable delivery must not bypass query authorization through raw outbox access.

## Operational safeguards and required evidence

Use least-privilege source reads and a separate owned database role. Keep secrets
outside git, redact logs and traces, and avoid party IDs or private payloads in
metrics labels and public fixtures. Enforce query cost, page-size and stream
resource limits. Never fall back to unfiltered upstream reads.

Verification must cover A, B and AB principals; individual and derived records;
counts, filters, pagination, joins and streams; cursor tampering/reuse; token and
source authorization expiry; revocation during replay and connected delivery;
and errors/logs that could expose private information.

## Revocation, cursors and operations

Bind internal checkpoints, caches and public opaque cursors to the appropriate
source, schema and authorization context. Never expose internal scope fingerprints
as permission-bearing public identifiers. Cursors must not be transferable across
principals or scope epochs. Recheck access when returning pages and delivering
stream events, including after reconnect.

A local grant change must immediately deny newly forbidden reads and invalidate
affected cursors, streams and caches. Source authorization failures fail closed.
Use a bounded revalidation lease; unknown or expired authorization cannot continue
serving private data. Instant upstream revocation detection during a disconnected
source cannot be promised. The exact lease and historical-retention/erasure
semantics must be settled before copying live private payloads.

Retained history and outbox entries are subject to current authorization, not
only the permissions at ingestion time. Never equate upstream pruning, loss of
visibility or a missing page with a verified contract archive.

Enforce query cost/page limits and protect metadata and error paths as well as
payloads. Secrets and private payloads must not appear in logs, metrics labels,
committed fixtures or diagnostic output. Public access defaults to denied.


## Observed source revocation — 2026-11-17

The [owned A/B/AB experiment](visibility.md) verified that fresh Ledger API reads
respect revoked grants even with a preissued token. The read-only source principal
can query its own current rights; user metadata resourceVersion must not be used
as a grant revision because grant/revoke operations do not change it in the tested
API. Compare the normalized grant content instead.

PQS readiness became false after a delay while its cached B-only record remained.
Therefore the first Rust serving path must require current caller grants and a
bounded source-authorization lease before using SQL results. The PQS data reader
and the read-only Ledger API authorization probe have separate credentials; this
probe is not a second ledger-data ingestion connector. Failure to renew the lease
must deny private serving. These requirements still need implementation and tests;
connection readiness alone is insufficient, and cached retention is not an archive.


## Rust core decision model

The first `cantongrid-core` slice implements the visibility conjunctions, context
matching and bounded source-lease decision rules. It rejects absent grants,
nonpositive/excessive lease lifetimes and expired credentials. A read must match
the caller/source party intersection for every contributing input and precede
both lease and credential expiry. Known invalidation denies unchanged cached rows.

The initial maximum lifetime is 15 seconds, tested with an injected monotonic
instant and no sleeps. This is a core policy choice, not a deployed SLA or a
completed source verifier. The HTTP grant probe, authenticated caller mapping,
generation changes, renewal, invalidation distribution and API checks must still
be wired and verified before any serving guarantee is made.
