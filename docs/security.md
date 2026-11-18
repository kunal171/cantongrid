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
  complexity 100, concurrent requests 16. No mutations, subscriptions or introspection.

## Operations

Keep source SQL SELECT-only and separate from the owned destination. Do not log
credentials, private payloads or party IDs, or put them in metrics labels and
public fixtures. Never fall back to broader/unfiltered source reads. Review scope
and generation changes before reusing a destination. Applied migrations are
immutable.

[Verification](verification.md) records LocalNet controls and remaining limits.
Report vulnerabilities through [SECURITY.md](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md).
