# Historical canonical state

Available in 0.4.0-alpha.6. Queries reconstruct immutable
contract membership at a source-local offset using retained create/archive evidence.
They enforce current organization grants and source authorization.

```graphql
query HistoricalContracts($offset: String!, $cursor: String) {
  contractsAsOf(asOfOffset: $offset, limit: 20, cursor: $cursor) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

REST uses `GET /v1/contracts/as-of?asOfOffset=100&limit=20`. Authenticate using the
same organization bearer as other [API queries](api.md). Offsets and counts are
exact decimal strings, not GraphQL integers or JavaScript numbers. Cursors are
signed and bound to the caller, current policy, selected source and requested offset.

A valid result has `coverage: observed_retained` and
`completeWithinConfiguredScope: true`. This says nothing about other parties,
templates, participants or pre-snapshot history. `historyFromOffset` is the common
snapshot floor for the current selection. Additive package baselines can raise it.
The offset names a completed source boundary, not indexing time or application time.

Unavailable history returns HTTP 409 with `coverage: unavailable`; GraphQL returns
an error carrying the same coverage extension. No confident zero total is returned
for pre-snapshot, future/unacknowledged, bootstrapping, missing or unsupported history.
Source outages and revoked grants also stop delivery.

This bounded prototype checks at most 4,096 journal pages, 8,192 canonical events
and 64 package baselines per query. Exceeding these budgets is unavailable, not a
truncated success. Existing source history can be used without a new migration,
but arbitrary/legacy fixture position formats do not prove temporal coverage.

The API returns canonical payloads, retaining their exact values and encoding.
It does not imply which model binding was active historically. Historical model
projections, aggregates, joins, wall-clock queries and application-effective-time
inference are outside this prototype. Current model publication/backfill does not
rewrite the canonical history used here.

Owned PostgreSQL/mock-source checks cover visibility, exact values, same-offset
archive/recreate behavior, replay/backfill consistency, REST/GraphQL parity,
missing coverage, cursor isolation, source outage and permission removal. These
checks are not new live-ledger or remote acceptance.
