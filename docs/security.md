# Authorization and private data

## Source and caller access

The Ledger credential authorizes ingestion. A separate organization HS256 bearer
identifies an API caller; its current grants must be a nonempty subset of the
configured source parties. Tokens, filters and model definitions cannot add rights.
The current profile is one organization on numeric loopback with the default
identity provider. See [API setup](guide/api.md).

The source checker requires an active user and the exact configured CanReadAs set.
Leases last at most 15 seconds and cannot outlive the credential. Failed renewal
clears cached authorization. Unknown rights or unhealthy PQS deny copying and
serving even when SQL still contains rows. Remote revocation is not instantaneous
during disconnection. Rights are compared directly; the tested Ledger
resourceVersion did not change when rights changed.

## Visibility and delivery

SQL applies source/caller visibility before counts and pagination. All private
transports check current source/caller authorization before and after response
encoding; a failed final guard discards the whole body. Local grants reload for
each request; removing or disabling a principal denies its existing bearer.

A shared A-or-B contract differs from a row combining A-only and B-only inputs:
the latter requires both permissions. Future joins, aggregates and streams must
preserve every input's visibility dependencies.

Schema041 archives have empty witnesses. The prior-create policy requires an
observed matching creation and current rights; absence, pruning or revocation
never fabricates an archive. History is reauthorized on read. Automatic erasure
is not implemented.

## Tokens and limits

- API and cursor keys are distinct, at least 32 bytes and different from the source
  credential. Issuer/audience match exactly; bearer lifetime is at most one hour.
- Signed cursors bind principal, source/plan, grants, query and expiry; model cursors
  also bind namespace, revision and normalized filters/sort. They last at most
  60 seconds and no longer than the bearer. They are signed, not encrypted.
- Responses use `Cache-Control: no-store`. Public records/errors omit operator
  witness sets, persistence envelopes and internal source keys.
- Pages contain 1–25 rows; requests are limited to 16 KiB and complete encoded
  responses to 2 MiB. GraphQL depth is 8, complexity 100; model operations have at
  most two row resolvers. Mutations, subscriptions and introspection are disabled.
- Admission occurs before body extraction: at most 16 concurrent requests with a
  30-second deadline, plus at most 16 database tasks. Cancellation does not release
  a database slot until its blocking task ends.

## Operator boundary

Registration, installation and rebuild commands require the owned database role;
there are no public registry/job mutation APIs. Namespaces and parent-contract
foreign keys preserve identity, not permission. Model reads join canonical
witnesses; staged rows remain private until guarded activation.

Destination checks reject nonowner schema, relation, sequence and column grants.
Projection checks also verify the compiled physical schema. Activation and model
writes share the canonical source lock; failed evaluation, drift or expired
leases roll back progress. Applied migrations are immutable.

The database owner and local operator remain trusted and can alter SQL/configuration.
Keep source SQL SELECT-only and separate from the destination. Store credentials
and generated state in operator-controlled directories; never log credentials,
private payloads or party IDs. Helpers use loopback without proxies/redirects and
atomic 0600 file replacement. Never broaden source access to recover availability.

[Verification](verification.md#security-regressions) records the review findings,
controls and limitations. PostgreSQL support is tracked in
[operations](guide/operations.md#postgresql-support).
Report vulnerabilities through [private reporting](https://github.com/kunal171/cantongrid/blob/main/SECURITY.md).
