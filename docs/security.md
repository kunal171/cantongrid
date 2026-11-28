# Security

Report vulnerabilities through
[private reporting](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md).

## Two independent authorities

- **Source:** the Ledger credential authorizes ingestion.
  - **Rights check:** it requires an active user and exactly the configured
    CanReadAs set.
  - **Leases:** they last at most 15 s and never outlive the credential.
  - **Failed renewal:** clears the cached authorization.
  - **Unknown rights or unhealthy PQS:** copying and serving are denied, even if
    the rows are still in SQL.
  - **Disconnection:** remote revocation isn't instant while disconnected.
- **Caller:** a separate organization HS256 bearer identifies the API caller.
  - **Grants:** the caller's current grants must be a non-empty subset of the
    source parties.
  - **No widening:** tokens, filters and model definitions can't add rights.
  - **Reloading:** grants reload on every request, so disabling a principal denies
    its existing bearer.

**Current profile:** one organization, numeric loopback and the default identity
provider.

## Visibility

- **Order:** SQL applies source and caller visibility before counts and paging.
- **Final guard:** every private transport checks authorization before and after
  encoding. If the final guard fails, the whole body is discarded.
- **Combined rows:** a row built from A-only and B-only inputs needs both
  permissions. That's different from a shared contract visible to A or B.
  - **Joins:** they check both endpoints before counts, fanout and encoding.
  - **Join cursors:** they hash only the visible result, so hidden updates can't
    invalidate them.
  - **Aggregates:** they require the full source scope.
- **Archives:** an archive needs a create that was observed earlier, plus current
  rights. Absence, pruning or revocation never fabricates an archive.
- **History:** reauthorized on every read. There's no automatic erasure.
- **Delivery:**
  - **Private IDs:** subscriptions and receipts use random IDs. JWT signatures
    don't encrypt claims, so global outbox positions and baseline offsets never
    appear in cursors or responses.
  - **Gaps:** pruning only hidden events never creates a public gap.
- **Upgrades:** while an upgrade publishes or cancels, reads, discovery, model
  queries and ingestion on that stream are gated.
  - **Staged pages:** they stay private.

## Tokens and limits

| Control | Value |
| --- | --- |
| API and cursor keys | Distinct, at least 32 bytes, different from the source credential |
| Bearer | Exact issuer and audience, at most 1 h |
| Cursors | Signed (not encrypted), bound to principal, source, grants, query and expiry; at most 60 s |
| Responses | `Cache-Control: no-store`; no witness sets, envelopes or internal keys |
| Page / request / response | 1–25 rows / 16 KiB / 2 MiB |
| GraphQL | Depth 8, complexity 100, 2 model row resolvers; no mutations, subscriptions or introspection |
| Admission | 16 concurrent requests (counted before reading the body), 30 s deadline, 16 database tasks |
| Long polls | Separate admission limits, reauthorized every second |

## Operator boundary

- **Trusted roles:** the database owner and the local operator are trusted.
  - **Owner-only commands:** registration, installation and rebuilds need the owned
    database role. There's no public API for registry changes.
- **Destination checks:**
  - **Grants:** grants to anyone other than the owner on schemas, relations,
    sequences or columns are rejected, but never revoked automatically.
  - **Projections:** the compiled projection schema is verified.
- **Rollback:** failed evaluation, drift or an expired lease rolls back progress.
- **Migrations:** applied migrations are immutable.
- **Source SQL:** keep it SELECT-only and separate from the destination.
- **Secrets:** never log credentials, payloads or party IDs. Never widen source
  access to restore availability.
- **Helpers:** use loopback with no proxies or redirects, and write files with
  atomic 0600 replacement.
- **Backups:** they contain private data, so keep them owner-only.

Remote preflight checks HTTPS, grants and restricted access. It doesn't prove
network identity.

Fixed findings and their controls are listed in
[verification](verification.md#security-regressions-fixed-in-alpha5).
