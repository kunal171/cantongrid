"""HTTP helpers exclusively for the disposable localhost Canton network."""
import base64
import hashlib
import hmac
import json
import time
from urllib.request import Request, urlopen


def token(subject='ledger-api-user', lifetime=300):
    if not 1 <= lifetime <= 3600:
        raise ValueError('Local development token lifetime must be between 1 and 3600 seconds')
    def encode(value):
        return base64.urlsafe_b64encode(value).rstrip(b'=').decode()
    header = encode(json.dumps({'alg': 'HS256', 'typ': 'JWT'}).encode())
    claims = encode(json.dumps({'sub': subject, 'aud': 'https://canton.network.global',
                                'exp': int(time.time()) + lifetime}).encode())
    signature = encode(hmac.new(b'unsafe', f'{header}.{claims}'.encode(), hashlib.sha256).digest())
    return f'{header}.{claims}.{signature}'


def request(base, path, bearer=None, *, body=None, method=None,
            content_type='application/json', read_json=True, timeout=30):
    if base not in {f'http://127.0.0.1:{prefix}{suffix}'
                    for prefix in (2, 3, 4) for suffix in ('975', '903')}:
        raise ValueError('Development requests must target a known loopback LocalNet API')
    if not path.startswith('/') or path.startswith('//'):
        raise ValueError('Expected an absolute API path')
    headers = {'Authorization': f'Bearer {bearer}'} if bearer else {}
    if body is not None:
        headers['Content-Type'] = content_type
        if not isinstance(body, bytes):
            body = json.dumps(body).encode()
    with urlopen(Request(base + path, data=body, headers=headers, method=method), timeout=timeout) as response:
        payload = response.read()
        return json.loads(payload) if read_json and payload else None


def get(base, path, bearer=None, read_json=True):
    return request(base, path, bearer, read_json=read_json)
