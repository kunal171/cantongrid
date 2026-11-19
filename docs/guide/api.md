# API authentication and queries

Start the [LocalNet worker](localnet-source.md) and serve on the default
`127.0.0.1:8080` bind. Source credentials and API caller identity are independent.
The server has no login/token-issuing endpoint.

## Keys and grants

Generate local keys once and supply the same values to the relevant processes:

```bash
export CANTONGRID_API_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export CANTONGRID_CURSOR_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
```

Each key must contain at least 32 bytes and differ from the other key and source
credential. Keep keys out of git and browsers. In `.local/my-app/grants.json`:

```json
{
  "version": 1,
  "epoch": 1,
  "principals": {
    "alice": {"enabled": true, "parties": ["replace-with-authorized-party"]}
  }
}
```

Replace the party placeholder with an exact source-scope member. `alice` is the
bearer's subject, not a Canton party. Grants are a nonempty, duplicate-free subset.
Disable/remove a principal to revoke access. For updates, increment `epoch` and
atomically replace the owner-only file; old cursors become invalid.

Use an organization-issued HS256 bearer with exact configured issuer/audience,
`sub`, `iat` and `exp`. Lifetime is at most one hour, with no expiry leeway.
For a local operator demo, this standard-library snippet creates a 15-minute bearer:

```bash
export CANTONGRID_CALLER_TOKEN="$(python3 - <<'PYTOKEN'
import base64
import hashlib
import hmac
import json
import os
import time


def encode(value):
    """Encode compact JSON as an unpadded JWT segment."""
    raw = json.dumps(value, separators=(',', ':')).encode()
    return base64.urlsafe_b64encode(raw).rstrip(b'=')


now = int(time.time())
message = encode({'alg': 'HS256', 'typ': 'JWT'}) + b'.' + encode({
    'sub': 'alice', 'iss': 'your-organization', 'aud': 'cantongrid-api',
    'iat': now, 'exp': now + 900,
})
signature = hmac.new(os.environ['CANTONGRID_API_KEY'].encode(), message, hashlib.sha256).digest()
print((message + b'.' + base64.urlsafe_b64encode(signature).rstrip(b'=')).decode())
PYTOKEN
)"
```

Match issuer/audience to runtime JSON. This is a local example, not an issuer
service. Do not print the resulting bearer into reports. The server also needs
current source rights, healthy PQS and an initialized destination.

## REST

```bash
curl --fail-with-body --silent --show-error \
  --get 'http://127.0.0.1:8080/v1/contracts' \
  --data-urlencode 'limit=10' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN"
```

Use `/v1/events` for lifecycle history. Both endpoints return `items`,
`visibleTotal`, `nextCursor` and `provenance`. The total is a caller-scoped decimal
string. To continue, put the returned cursor in `NEXT_CURSOR` and add
`--data-urlencode "cursor=$NEXT_CURSOR"` to the same endpoint. Stop at null.
Limits are 1–25, default 20. There is no party override or arbitrary SQL filter.

## GraphQL

Save `.local/my-app/contracts.graphql`:

```graphql
query Contracts($limit: Int, $cursor: String) {
  activeContracts(limit: $limit, cursor: $cursor) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

Package and send it without hand-escaping the query:

```bash
python3 - <<'PYREQUEST'
import json
from pathlib import Path

path = Path('.local/my-app')
request = {'query': (path / 'contracts.graphql').read_text(),
           'variables': {'limit': 10, 'cursor': None}}
(path / 'contracts-query.json').write_text(json.dumps(request, indent=2) + '\n')
PYREQUEST
curl --fail-with-body --silent --show-error 'http://127.0.0.1:8080/graphql' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN" \
  --header 'Content-Type: application/json' \
  --data-binary @.local/my-app/contracts-query.json
```

Replace `activeContracts` with `lifecycleEvents` for history, using its own cursor.
`items` and `provenance` are JSON scalars, not nested typed fields. `/graphql`
exposes generic roots; use `/models/graphql` for generated model queries below.
Check GraphQL `errors` even on HTTP 200. Mutations, subscriptions and introspection
are disabled.

## Results and pagination

Contract payloads use tagged `raw_json` with exact inner source text. Preserve
Int64, Numeric and offset strings; do not convert financial values to JavaScript
`Number`. Use exact integer/decimal handling when calculations are needed.

Cursors bind principal, grants, query kind and expiry. They last at most 60 seconds
and no longer than the bearer. After expiry or grant changes, start a new page
sequence. Never edit them or transfer them between users. Generic contract/history cursors
remain endpoint-specific; matching model queries can share cursors as described below.

Provenance reports source/network/generation, `sourceLocalWatermark`,
`historyFromOffset`, `observedAgeSeconds` and coverage (`bootstrapping`,
`partial_history`, `unknown`). `recently_observed` means within 30 seconds, not
zero ledger lag. Responses are `no-store`; limits and policy are in
[security](../security.md).

## Generated model queries

Typed model operations use `POST /models/graphql`.
Use `models graphql` for the model SDL and `models typescript` for exact transport
interfaces. `/graphql` above retains its generic contract/history schema.
Model operations use the same independent caller/source checks and no-store policy,
with a maximum of two model resolvers and 25 rows/page.

## Filter and sort models

Discover active bindings with authenticated `GET /v1/models`. It returns
`{"models": [...]}`. Each entry contains namespace, name, revision, fingerprint,
`queryField` and output field types/operators. `GET /v1/models/demo/records`
returns one entry. The GraphQL `models` field exposes the same metadata.
This is the organization's active catalog; listing a model does not grant access
to its rows. Definitions, expressions, literal values and source paths stay private.

Query the bound Visibility example through REST:

```bash
curl --fail-with-body --silent --show-error \
  'http://127.0.0.1:8080/v1/models/demo/records/query' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN" \
  --header 'Content-Type: application/json' \
  --data '{"limit":10,"filters":{"amount":{"gte":"1.20"},"note":{"isNull":false}},"sort":{"field":"amount","direction":"desc"}}'
```

For GraphQL, replace `MODEL_QUERY_FIELD` below with discovery's `queryField`
and send the operation to `/models/graphql` using the request packaging above:

```graphql
query FilteredRecords($cursor: String) {
  MODEL_QUERY_FIELD(
    namespace: "demo"
    limit: 10
    cursor: $cursor
    filters: { amount: { gte: "1.20" }, note: { isNull: false } }
    sort: { field: "amount", direction: desc }
  ) {
    visibleTotal
    nextCursor
    fingerprint
    items {
      contractId
      values { amount sequence note }
    }
  }
}
```

`cantongrid models graphql <models.json>` and `cantongrid models typescript
<models.json>` emit the filter/sort input definitions and TypeScript query
interfaces. Both transports use these rules:

- At most eight predicates, combined with AND. All scalar fields support `eq`
  and `ne`; only Int64/Numeric support `gt`, `gte`, `lt`, `lte`.
- Use `isNull: true` or `false` for optional fields. Other comparisons exclude
  null rows, including `ne`. Null comparison arguments are rejected.
- Financial and integer arguments are strings; booleans are JSON/GraphQL booleans.
  Decimal scale is checked before SQL casts: `1.2` at scale 2 becomes `1.20`,
  while `1.234` fails. Filter strings are limited to 1,024 UTF-8 bytes.
- Sort one declared output field, `asc` by default. Nulls come last in either
  direction; equal values use contract ID ascending. Without a sort field,
  ordering is contract ID ascending. Numeric ordering uses native PostgreSQL types.
- Counts include visibility and filters, before pagination. Page sizes remain
  1–25, default 20. Each request reads one database snapshot; subsequent pages
  reflect later ingestion and archives rather than retaining that snapshot.

Pass the same filters/sort with the returned cursor. It binds normalized options,
model revision, namespace, principal, grants, source and expiry. Equivalent decimal
spellings share a query identity. Matching REST and model GraphQL queries can share
cursors; changed options, promotions, revoked grants or invalid tokens fail closed.
When upgrading from alpha.2, restart model pagination; the older cursor format
is invalid.

REST returns 400 for invalid query input/cursors, 404 for unknown bindings,
401 for invalid callers and 503 for source/schema outages. Overload returns 429;
the request deadline returns 504. GraphQL clients must also check `errors`.
[Security](../security.md#tokens-and-limits) lists transport budgets. Sort keys
that cannot fit the 4 KiB internal cursor fail without a partial page. No
automatic indexes are created. Recorded query acceptance is in
[verification](../verification.md#recorded-localnet-acceptance).
