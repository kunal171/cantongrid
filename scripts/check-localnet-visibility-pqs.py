#!/usr/bin/env python3
"""Observe AB source cache and a bounded live-stream revocation window."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import urlopen
from localnet_api import get, request, token
from localnet_visibility import AMOUNT, BASE, SEQUENCE, USERS, acs, context, right, rights_equal, write_private, DIRECTORY

ROOT = Path(__file__).resolve().parents[1]
ENV = {**os.environ, 'PQS_UID': str(os.getuid()), 'PQS_GID': str(os.getgid())}
DATABASE = ['docker', 'compose', '-p', 'cantongrid-pqs', '-f', str(ROOT / 'dev/pqs.compose.yaml'),
            'exec', '-T', 'pqs-postgres', 'psql', '-X', '-qAt', '-U', 'pqs_owner',
            '-d', 'cantongrid_visibility', '-v', 'ON_ERROR_STOP=1']


def cached():
    sql = '''BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT json_agg(json_build_object('payload', payload, 'witnesses', witnesses))
FROM active('Visibility:Record');
COMMIT;'''
    result = subprocess.run(DATABASE, input=sql, text=True, capture_output=True, check=True, env=ENV)
    return json.loads(result.stdout)


def ready():
    try:
        with urlopen('http://127.0.0.1:18081/readyz', timeout=3) as response:
            data = json.load(response)
        return data.get('status') == 'ok' and all(data.get(k) is True for k in
                                                ('grpc_up', 'jdbc_connection_pool_up', 'stream_up'))
    except (OSError, ValueError):
        return False


def main():
    parties = context()['parties']
    if not ready():
        raise ValueError('Start the dedicated AB source and wait for readiness before this test')
    rows = cached()
    expected = {'a-only': {parties['a']}, 'b-only': {parties['b']}, 'joint': set(parties.values())}
    if len(rows) != 3 or {r['payload']['label'] for r in rows} != set(expected):
        raise ValueError('AB cache does not contain exactly the controlled fixture records')
    for row in rows:
        value = row['payload']
        if (set(row['witnesses']) != expected[value['label']] or value['amount'] != AMOUNT
                or value['sequence'] != SEQUENCE or value['note'] is not None or value['state'] != 'Ready'):
            raise ValueError('PQS witness evidence or exact payload values differ from the source')
    isolation = subprocess.run([
        'docker', 'compose', '-p', 'cantongrid-pqs', '-f', str(ROOT / 'dev/pqs.compose.yaml'),
        'exec', '-T', '-e', 'PGPASSWORD=local-pqs-reader', 'pqs-postgres',
        'psql', '-X', '-h', '127.0.0.1', '-U', 'cantongrid_pqs_reader', '-d', 'cantongrid_visibility',
        '-c', 'SELECT 1;'], capture_output=True, text=True, env=ENV)
    if isolation.returncode == 0 or 'permission denied for database' not in isolation.stderr:
        raise ValueError('Original source reader was not denied the separate AB database')
    print('PASS: exact AB payloads and witness sets; original SQL reader cannot connect to the AB store', flush=True)

    stale = token(USERS['ab'])
    path = '/v2/users/' + USERS['ab'] + '/rights'
    change = {'userId': USERS['ab'], 'identityProviderId': '',
              'rights': [right('CanReadAs', parties['b'])]}
    result = {'observation_window_seconds': 30}
    try:
        request(BASE, path, token(), method='PATCH', body=change)
        started = time.monotonic()
        if not rights_equal(get(BASE, path, stale)['rights'], [right('CanReadAs', parties['a'])]):
            raise ValueError('Preissued source credential did not report current narrowed rights')
        try:
            acs(USERS['ab'], [parties['b']], stale)
        except HTTPError as exc:
            if exc.code != 403:
                raise
        else:
            raise ValueError('Fresh B snapshot was accepted after revocation')
        lost = None
        while time.monotonic() - started < 30:
            if not ready():
                lost = round(time.monotonic() - started, 2)
                break
            time.sleep(1)
        retained = cached()
        result.update(source_readiness_lost_after_seconds=lost,
                      cache_rows_after_revocation=len(retained),
                      cached_b_only_retained=any(r['payload']['label'] == 'b-only' for r in retained),
                      preissued_self_rights_narrowed=True, new_b_snapshot_denied_with_preissued_token=True)
    finally:
        request(BASE, path, token(), body=change)
        subprocess.run([sys.executable, str(ROOT / 'scripts/localnet-visibility-pqs.py'), 'up'],
                       check=True, env=ENV, stdout=subprocess.DEVNULL)
    write_private(DIRECTORY / 'revocation-observation.json', result)
    print(json.dumps(result, indent=2))
    print('PASS: fixture grant restored; AB source recreated with a renewed token')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'Visibility source check failed: {exc}', file=sys.stderr)
        sys.exit(1)
