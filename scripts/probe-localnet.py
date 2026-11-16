#!/usr/bin/env python3
"""Read-only health and authenticated Ledger API checks for our disposable LocalNet."""
import base64
import hashlib
import hmac
import json
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


def token():
    def encode(value):
        return base64.urlsafe_b64encode(value).rstrip(b'=').decode()
    header = encode(json.dumps({'alg': 'HS256', 'typ': 'JWT'}).encode())
    claims = encode(json.dumps({'sub': 'ledger-api-user',
                                'aud': 'https://canton.network.global',
                                'exp': int(time.time()) + 300}).encode())
    signature = encode(hmac.new(b'unsafe', f'{header}.{claims}'.encode(), hashlib.sha256).digest())
    return f'{header}.{claims}.{signature}'


def get(base, path, bearer=None):
    headers = {'Authorization': f'Bearer {bearer}'} if bearer else {}
    with urlopen(Request(base + path, headers=headers), timeout=10) as response:
        body = response.read()
        return json.loads(body) if body else None


def main():
    bearer = token()  # Public upstream dev credential, never a shared-network token.
    for name, prefix in [('app-user', 2), ('app-provider', 3), ('sv', 4)]:
        base = f'http://127.0.0.1:{prefix}975'
        get(f'http://127.0.0.1:{prefix}903', '/api/validator/readyz')
        get(base, '/readyz')
        try:
            get(base, '/v2/parties')
        except HTTPError as exc:
            if exc.code != 401:
                raise
        else:
            raise ValueError(f'{name}: expected unauthenticated party request to fail with 401')
        version = get(base, '/v2/version', bearer)
        end = get(base, '/v2/state/ledger-end', bearer)
        parties = get(base, '/v2/parties', bearer)
        if not isinstance(parties.get('partyDetails'), list):
            raise ValueError(f'{name}: unexpected party-list response shape')
        print(f'{name}: ready; unauthenticated request rejected; '
              f'version={version.get("version")}; offset={end.get("offset")}; '
              f'party page count={len(parties["partyDetails"])}')
    print('PASS: all three validators and authenticated read-only Ledger APIs')


if __name__ == '__main__':
    try:
        main()
    except (HTTPError, URLError, OSError, ValueError) as exc:
        print(f'LocalNet probe failed: {exc}', file=sys.stderr)
        sys.exit(1)
