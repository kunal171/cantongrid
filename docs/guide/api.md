# API

`cantongrid serve` listens on `127.0.0.1:8080`.
- **Two identities:** source credentials and API caller identity are separate.
- **No login:** the server has no login or token endpoint.
- **Before you start:** run the [LocalNet worker](localnet.md) first.

## Keys and grants

```bash
export CANTONGRID_API_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export CANTONGRID_CURSOR_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
```

- **Key rules:** each key is at least 32 bytes and differs from the other key
  and from the source credential.
- **Where keys live:** keep keys out of git and browsers.

`grants.json` maps bearer subjects (not Canton parties) to parties in the source
scope:

```json
{ "version": 1, "epoch": 1,
  "principals": { "alice": { "enabled": true, "parties": ["<authorized party>"] } } }
```

- **Revoking:** disable or remove a principal.
- **Updating:** increase `epoch` and replace the file atomically. Old cursors
  stop working.

**Bearer tokens.**
- **Format:** callers send an HS256 bearer from your organization, with the exact
  issuer and audience, `sub`, `iat` and `exp`.
- **Lifetime:** at most 1 hour, with no expiry leeway.
- **Generating:** CantonGrid doesn't issue tokens. For local tests, sign one
  yourself with `CANTONGRID_API_KEY` (standard JWT HS256).

## Generic contracts and history

```bash
curl --fail-with-body -sS --get 'http://127.0.0.1:8080/v1/contracts' \
  --data-urlencode 'limit=10' -H "Authorization: Bearer $CANTONGRID_CALLER_TOKEN"
```

- **Endpoints:**
  - **REST:** `/v1/contracts` for active contracts and `/v1/events` for history.
  - **GraphQL:** `POST /graphql` with `activeContracts(limit, cursor)` and
    `lifecycleEvents(limit, cursor)`.
- **Response:** each returns `items`, `visibleTotal` (a decimal string),
  `nextCursor` and `provenance`.
- **Paging:** pass `cursor` until it is null. Pages hold 1–25 items (default 20).
- **No overrides:** there's no party override and no SQL filter.
- **GraphQL:**
  - **Disabled:** mutations, subscriptions and introspection.
  - **Errors:** check `errors` even on HTTP 200.

**Payloads.**
- **Encoding:** payloads are tagged `raw_json` and keep the source text exactly.
- **Numbers:** keep Int64 and Numeric values as strings, and never convert them
  to a JavaScript `Number`.

**Cursors.**
- **Binding:** cursors are tied to the principal, grants, query and expiry.
- **Lifetime:** at most 60 s, and never longer than the bearer.
- **Rules:** never edit a cursor or share one between users.

**Provenance.**
- **Source:** source, network and generation.
- **Progress:** `sourceLocalWatermark` and `historyFromOffset`.
- **Freshness:** `observedAgeSeconds`.
- **Coverage:** `bootstrapping`, `partial_history` or `unknown`.

`recently_observed` means within 30 s; it doesn't mean zero ledger lag.

## Model queries

- **Catalog:** `GET /v1/models` lists active bindings with their `queryField`,
  output types and operators. Being listed doesn't grant access to rows.
- **Query:** `POST /v1/models/{namespace}/{name}/query`, or
  `POST /models/graphql` with the generated root.
  - **Schemas:** `models graphql` and `models typescript` emit the SDL and the
    TypeScript interfaces.
  - **Limits:** at most 2 model resolvers per request and 25 rows per page.

```bash
curl --fail-with-body -sS 'http://127.0.0.1:8080/v1/models/demo/records/query' \
  -H "Authorization: Bearer $CANTONGRID_CALLER_TOKEN" -H 'Content-Type: application/json' \
  --data '{"limit":10,"filters":{"amount":{"gte":"1.20"},"note":{"isNull":false}},
           "sort":[{"field":"amount","direction":"desc"}]}'
```

**Filters.**
- **Limit and logic:** up to 8 predicates, combined with AND.
- **Operators:** every scalar supports `eq` and `ne`. Int64 and Numeric also
  support `gt`, `gte`, `lt` and `lte`.
- **Nulls:** `isNull` handles optional fields, and other comparisons leave out
  null rows.
- **Numbers:** they are strings, and scale is checked (`1.2` becomes `1.20`, while
  `1.234` fails at scale 2).
- **Strings:** at most 1,024 bytes.

**Sorting.**
- **Keys:** a single object or a list of 1–4 keys. GraphQL uses `sort` or
  `orderBy`.
- **Ordering:** native exact ordering, nulls last, then contract ID as the
  tie-breaker.
- **Cursor size:** sort keys that can't fit the 4 KiB cursor fail.
- **Indexes:** none are created automatically.

**Counts and pages.**
- **Counts:** apply visibility and filters, before pagination.
- **Snapshots:** each request reads one database snapshot. Later pages see later
  ingestion.

**Model cursors.**
- **Binding:** a model cursor is bound to the normalized options, model revision,
  principal, grants and source.
- **Sharing:** equivalent REST and GraphQL queries can share it.
- **Invalidation:** any change (options, a promotion, a revoked grant) fails closed.

| Status | Meaning |
| --- | --- |
| 400 | Bad input or cursor |
| 401 | Invalid caller |
| 404 | Unknown binding |
| 429 | Overloaded |
| 503 | Source or schema unavailable |
| 504 | Request deadline |

Every response is `no-store`. Transport budgets are in
[security](../security.md#tokens-and-limits).

## Other APIs

Each has its own guide:

| API | Guide |
| --- | --- |
| Lineage, relationships, joins, temporal reads | [composition](composition.md) |
| Aggregates (full source scope required) | [models](models.md#aggregates) |
| Durable subscriptions and bootstrap | [delivery](delivery.md) |
| Reconciliation, REST and `/reconciliation/graphql` (0.5 alpha) | [reconciliation](reconciliation.md) |
| Developer inspector | [operations](operations.md#inspector) |
