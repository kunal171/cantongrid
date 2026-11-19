"""Read-only health and authenticated Ledger API checks for our disposable LocalNet."""
from urllib.error import HTTPError
from ..ledger import get, token


def main():
    """Verify local services, anonymous denial and authenticated Ledger reads without submitting writes."""
    bearer = token()  # Public upstream dev credential, never a shared-network token.
    for name, prefix in [('app-user', 2), ('app-provider', 3), ('sv', 4)]:
        base = f'http://127.0.0.1:{prefix}975'
        get(f'http://127.0.0.1:{prefix}903', '/api/validator/readyz', read_json=False)
        get(base, '/readyz', read_json=False)
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
        if not version.get('version') or 'offset' not in end:
            raise ValueError(f'{name}: unexpected version/ledger-end response shape')
        if not isinstance(parties.get('partyDetails'), list):
            raise ValueError(f'{name}: unexpected party-list response shape')
        print(f'{name}: ready; unauthenticated request rejected; '
              f'version={version.get("version")}; offset={end.get("offset")}; '
              f'party page count={len(parties["partyDetails"])}')
    print('PASS: all three validators and authenticated read-only Ledger APIs')
