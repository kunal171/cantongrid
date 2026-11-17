# Authenticate API users

CantonGrid separates source access from application access. A source credential
authorizes ingestion; an organization bearer token identifies an API caller.
Current party permissions come from the grants file, not token-supplied parties.

## Configure organization keys

The runtime example refers to these environment variables:

```bash
export CANTONGRID_API_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
export CANTONGRID_CURSOR_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(32))')"
```

Run this once for the local session and supply the same values to each relevant
process. Do not regenerate keys between issuing a bearer and starting the server.
Keys must differ from each other and the source credential, and each contain at
least 32 bytes. Store shared deployment credentials through your operator's secret
management; do not commit an environment file or send these keys to the browser.

## Map a principal to permitted parties

Edit `.local/my-app/grants.json`:

```json
{
  "version": 1,
  "epoch": 1,
  "principals": {
    "alice": {
      "enabled": true,
      "parties": ["replace-with-authorized-party"]
    }
  }
}
```

Replace the party placeholder with an exact member of the runtime source scope.
Each enabled principal needs a nonempty, duplicate-free subset. `alice` is the
token subject, not a Canton party. The server reloads grants for each query.
To revoke access, disable/remove the principal. To change grants, increment
`epoch` and atomically replace the file using a completed owner-only temporary
file in the same directory. Existing cursors become invalid when grants change.

## Obtain a caller bearer

Use your trusted organization's issuer. The supported token contract is:

| JWT field | Requirement |
| --- | --- |
| Header `alg` | `HS256` |
| `iss` | Exact `api.issuer`, e.g. `your-organization` |
| `aud` | Exact `api.audience`, e.g. `cantongrid-api` |
| `sub` | Enabled principal in the current grants file |
| `iat` | Current or earlier Unix seconds |
| `exp` | Future Unix seconds, at most one hour after `iat` |

There is no token-issuing/login endpoint in CantonGrid. For a local operator demo,
the following standard-library Python snippet creates a 15-minute test bearer
using the existing key. It is a development example, not an issuer service:

```bash
export CANTONGRID_CALLER_TOKEN="$(python3 - <<'PY'
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
header = encode({'alg': 'HS256', 'typ': 'JWT'})
claims = encode({
    'sub': 'alice',
    'iss': 'your-organization',
    'aud': 'cantongrid-api',
    'iat': now,
    'exp': now + 900,
})
message = header + b'.' + claims
signature = hmac.new(os.environ['CANTONGRID_API_KEY'].encode(), message, hashlib.sha256).digest()
print((message + b'.' + base64.urlsafe_b64encode(signature).rstrip(b'=')).decode())
PY
)"
```

Match the example issuer/audience to your runtime configuration. Keep the bearer
in your local environment; do not print it into reports. `CANTONGRID_CALLER_TOKEN`
is a convenience variable for the curl examples, not a server configuration field.

Before running [queries](querying.md), the server also needs a healthy source,
current Ledger rights and an initialized destination. Cached SQL rows alone do
not authorize serving. Source authorization leases last at most 15 seconds;
instantaneous remote revocation during disconnection is not claimed.
