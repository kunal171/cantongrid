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
`items` and `provenance` are JSON scalars, not nested typed fields. There is no
`markets`/`records` root yet. Check GraphQL `errors` even on HTTP 200. Mutations,
subscriptions and introspection are disabled.

## Results and pagination

Contract payloads use tagged `raw_json` with exact inner source text. Preserve
Int64, Numeric and offset strings; do not convert financial values to JavaScript
`Number`. Use exact integer/decimal handling when calculations are needed.

Cursors bind principal, grants, query kind and expiry. They last at most 60 seconds
and no longer than the bearer. After expiry or grant changes, start a new page
sequence. Never edit or transfer them between users/endpoints.

Provenance reports source/network/generation, `sourceLocalWatermark`,
`historyFromOffset`, `observedAgeSeconds` and coverage (`bootstrapping`,
`partial_history`, `unknown`). `recently_observed` means within 30 seconds, not
zero ledger lag. Responses are `no-store`; limits and policy are in
[security](../security.md).
