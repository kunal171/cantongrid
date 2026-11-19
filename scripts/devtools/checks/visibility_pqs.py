"""Observe AB source cache and a bounded live-stream revocation window."""
from ..common import write_private
import json
import subprocess
import time
from urllib.error import HTTPError
from ..ledger import get, request, token
from ..visibility import AMOUNT, BASE, SEQUENCE, USERS, acs, context, right, rights_equal, DIRECTORY

from ..common import ENV, pg_command, source_ready
from ..pqs import up

DATABASE = pg_command('cantongrid_visibility')


def cached():
    """Inspect the isolated AB cache without treating retained SQL rows as current permission."""
    sql = '''BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT json_agg(json_build_object('payload', payload, 'witnesses', witnesses))
FROM active('Visibility:Record');
COMMIT;'''
    result = subprocess.run(DATABASE, input=sql, text=True, capture_output=True, check=True, env=ENV)
    return json.loads(result.stdout)


def ready():
    """Check the AB exporter rather than the unrelated provider profile."""
    return source_ready(visibility=True)


def main():
    """Observe cache retention and readiness after revocation, then restore rights and renew the source."""
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
    isolation = subprocess.run(pg_command('cantongrid_visibility', reader=True),
                               input='SELECT 1;', capture_output=True, text=True, env=ENV)
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
        up(visibility=True, quiet=True)
    write_private(DIRECTORY / 'revocation-observation.json', result)
    print(json.dumps(result, indent=2))
    print('PASS: fixture grant restored; AB source recreated with a renewed token')
