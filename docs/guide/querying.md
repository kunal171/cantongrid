# Query REST and GraphQL

Start the worker and server as described in [LocalNet setup](localnet-source.md)
and obtain a [caller bearer](authentication.md). These examples use the default
`127.0.0.1:8080` bind. Both transports share the same caller-scoped SQL read path.

## Active contracts over REST

```bash
curl --fail-with-body --silent --show-error \
  --get 'http://127.0.0.1:8080/v1/contracts' \
  --data-urlencode 'limit=10' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN"
```

Responses contain `items`, `visibleTotal`, `nextCursor` and `provenance`.
`visibleTotal` is a decimal string and counts only records visible to this caller.
`items` contains generic canonical records, not generated business-model objects.
The current payload uses the core's `raw_json` typed-value encoding: preserve the
inner source JSON text and parse with exact numeric handling when needed.

## Lifecycle history and pagination

```bash
curl --fail-with-body --silent --show-error \
  --get 'http://127.0.0.1:8080/v1/events' \
  --data-urlencode 'limit=10' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN"
```

If `nextCursor` is non-null, copy it unchanged to a local `NEXT_CURSOR` variable
and repeat the **same endpoint** with `--data-urlencode "cursor=$NEXT_CURSOR"`.
Stop when it is null. Contract and event cursors are separate, principal-bound,
signed tokens. They expire within 60 seconds and no later than the caller bearer.
After expiry or grant changes, restart pagination; do not decode/edit cursor state.

Both endpoints accept `limit` from **1 to 25**, default 20. They accept no party
override, arbitrary SQL filter or business-model query parameter.

## GraphQL with readable queries

Save the query as `.local/my-app/contracts.graphql`:

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

```bash
# Package the readable query into a JSON HTTP request without hand-escaping it.
python3 - <<'PY'
import json
from pathlib import Path

directory = Path('.local/my-app')
request = {
    'query': (directory / 'contracts.graphql').read_text(),
    'variables': {'limit': 10, 'cursor': None},
}
(directory / 'contracts-query.json').write_text(json.dumps(request, indent=2) + '\n')
PY
curl --fail-with-body --silent --show-error \
  'http://127.0.0.1:8080/graphql' \
  --header "Authorization: Bearer $CANTONGRID_CALLER_TOKEN" \
  --header 'Content-Type: application/json' \
  --data-binary @.local/my-app/contracts-query.json
```

Use the `lifecycleEvents` root for history, with its own cursor:

```graphql
query Lifecycle($limit: Int, $cursor: String) {
  lifecycleEvents(limit: $limit, cursor: $cursor) {
    items
    visibleTotal
    nextCursor
    provenance
  }
}
```

`items` and `provenance` are JSON scalars, so they take no nested GraphQL selection
set. Do not query `markets` or `records` roots from the offline model examples;
generated typed resolvers are not available yet. Inspect GraphQL `errors` even
when HTTP returns 200. Mutations, subscriptions and introspection are disabled.

## Interpret provenance

| Field | Meaning |
| --- | --- |
| `coverage` | `bootstrapping`, `partial_history` or `unknown`; never a global completeness claim |
| `sourceLocalWatermark` | Exact source-local position as a string; not an event count |
| `historyFromOffset` | Captured source boundary for available history |
| `observedAgeSeconds` | Time since a recorded observation, or null |
| `freshness` | `recently_observed` within 30 seconds, otherwise `stale_or_unknown` |
| `payloadEncoding` | `raw_json` for this profile |

Keep Int64, Numeric and offset strings exact. In JavaScript, use `BigInt` for
integer calculations and an exact decimal library for Numeric values; avoid
`Number(...)` on financial strings. Do not cache private responses across users.
Responses carry `Cache-Control: no-store`. Limits also include a 16-KiB request
body, depth 8, complexity 100 and a 2-MiB result-page bound.
