# Authorization and private data

## Source and caller access

The Ledger credential authorizes ingestion. A separate organization HS256 bearer
identifies an API caller; the current grants file maps that principal to a
nonempty subset of source parties. Tokens and request filters cannot add rights.
The supported deployment is one organization on numeric loopback, using the default
identity provider. See [setup](guide/api.md).

Source checks verify the active user and exact CanReadAs set. Leases last at most
15 seconds and cannot outlive the source credential. Failed renewal drops cached
authorization. Unknown rights or unhealthy PQS deny copying and serving even when
rows remain in SQL. Remote revocation is not instantaneous during disconnection.
Grant content is compared directly; the tested Ledger resourceVersion does not
change when rights are granted/revoked.

## Visibility

Source/caller intersection is applied in SQL before counts and pagination.
Queries recheck source and caller access before delivery, including after a
complete GraphQL operation. Local grants reload on each request. Disabling a
principal denies its existing bearer on the next request.

A joint contract visible to A or B differs from a derived row combining A-only
and B-only inputs: that row requires access to both inputs. Future joins,
aggregates and streams must retain these dependencies. Public schemas, errors,
counts, caches and cursor handling must not reveal inaccessible records.

Schema041 archives retain their empty source witness evidence. The supported
prior-create policy requires an observed matching creation and current rights.
Revocation, pruning and absence do not fabricate an archive. Retained history is
reauthorized on read; automatic erasure is not implemented.

## Tokens, cursors and responses

- API and cursor keys are separate, at least 32 bytes, and differ from the source
  credential. Issuer/audience are exact; bearer lifetime is at most one hour.
- Signed cursors bind principal, source/plan, grant fingerprint, query kind and
  expiry. They last at most 60 seconds and no longer than the bearer. They are
  opaque API tokens, not encryption; do not transfer them between callers.
- Counts/pages and public event provenance omit operator witness sets and internal
  source keys. Responses use `Cache-Control: no-store` and bounded public errors.
- API limits: pages 1–25, request body 16 KiB, result page 2 MiB, GraphQL depth 8,
  complexity 100, concurrent requests 16 and concurrent database tasks 16. Admission
  precedes body extraction; a 30-second deadline includes body reads and resolution.
  Cancelling a request does not release a running database task's slot. The 2 MiB
  cap applies to the complete encoded response, including multiple GraphQL roots.
  No mutations, subscriptions or introspection.
- Model registry commands are operator-only database operations, not HTTP endpoints.
  Registration/inspection summaries omit literals; definitions grant no data access.
- Table installation is also operator-only. It verifies schema/table ownership and
  rejects nonowner grants or schema drift. Parent-contract foreign keys preserve
  identity, not permission. Model reads join canonical witnesses
  before counts/pages, preserve exact numeric strings and revalidate bindings/grants
  before delivery. Namespace labels never grant access.

- Activation requires a verified physical source plan and exact current source-party lease.
  Backfill and live model writes share the canonical writer lock/transaction; failed
  evaluation, drift or expired leases stop progress. Bindings never grant API access.

## Operations

Keep source SQL SELECT-only and separate from the owned destination. Do not log
credentials, private payloads or party IDs, or put them in metrics labels and
public fixtures. Never fall back to broader/unfiltered source reads. Review scope
and generation changes before reusing a destination. Applied migrations are
immutable.

[Verification](verification.md) records LocalNet controls and remaining limits.
Report vulnerabilities through [SECURITY.md](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md).


## Repository security review — 2026-11-19

This code review and regression pass covers the Rust runtime, authorization,
PQS reader, model compiler/queries/rebuilds, destination guards, CLI, local helpers,
package policy and GitHub workflows. It is not an independent penetration test
or a production security certification. Fixes are in unreleased alpha.5.

| Finding | Impact and fix |
| --- | --- |
| Incomplete destination ACL checks | Public schema/table/sequence and column grants could escape the intended private-database boundary. Reopen/initialization now reject nonowner privileges on destination objects; projection checks also inspect column ACLs. |
| Admission after body extraction | Incomplete bodies could bypass the handler semaphore. Middleware now admits at most 16 requests before extraction and imposes a 30-second deadline. |
| Cancellation released capacity early | SQL work could outlive its HTTP permit. A separate permit now stays inside each blocking database task until it ends. |
| Generic GraphQL aggregate size | Multiple individually valid pages could exceed the advertised 2 MiB limit. All transports now share a bounded serialization writer for the whole response. |
| Broader PQS lease accepted | Library callers could supply a lease wider than the plan's configured parties. The reader now rejects any party-set mismatch before connecting. The runtime checker already required exact grants. |
| Local helper transport/file behavior | Default redirects/proxies could forward fixture bearers; in-place writes could follow links or leave partial grant files. Helpers now use numeric loopback without redirects/proxies and atomic 0600 file replacement, rejecting symlink destinations. |

The ACL findings required operator SQL privilege changes; no unprivileged HTTP
caller was shown to obtain another caller's records. Validation detects unsafe
ACL drift; it cannot undo access already granted outside CantonGrid. Review and
revoke unintended database grants before restarting. No applied migration was
changed and no new runtime dependency was introduced.

Controls include real PostgreSQL grant changes rolled back after each check,
real HTTP slow-body admission/timeout checks, cancelled blocking-task controls,
aggregate/escaped JSON limits and helper link/transport regressions. Existing
caller isolation, signed-cursor, revocation, source-outage and rebuild tests pass.
The blocking-task lifetime follows [Tokio's documented cancellation behavior](https://docs.rs/tokio/1.54.1/tokio/task/fn.spawn_blocking.html).

RustSec scanned 249 locked dependencies against 1,550 fetched advisories with no
findings. License/source/bans policy, redacted full-history and eligible-tree
secret scans, and actionlint passed. Duplicate-version notices for `base64`, `syn`
and `wasi` remain dependency-tree notices, not reported vulnerabilities. These
results are point-in-time checks; scheduled CI continues scanning.

Remaining boundaries: numeric-loopback LocalNet only; no remote/TLS or production
load testing, exhaustive fuzzing, or independent audit. Large staged LocalNet
acceptance remains pending. Remote revocation has the lease boundary described
above. The trusted database owner and local operator can alter stored data,
configuration and ACLs; private files must stay in operator-controlled directories.
