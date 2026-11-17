#!/usr/bin/env python3
"""Read-only SQL and source-grant checks for the scoped PQS development fixture."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.request import urlopen
from localnet_api import get, token

ROOT = Path(__file__).resolve().parents[1]
USER = 'cantongrid-pqs-app-provider'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--at-offset', type=int, help='Check a retained historical snapshot')
    args = parser.parse_args()
    if args.at_offset is not None and args.at_offset < 0:
        raise ValueError('Offset must be nonnegative')
    bound = str(args.at_offset) if args.at_offset is not None else 'latest_offset()'
    base = 'http://127.0.0.1:3975'
    admin = token()
    party = get(base, '/v2/users/ledger-api-user', admin)['user']['primaryParty']
    expected = [{'kind': {'CanReadAs': {'value': {'party': party}}}}]
    if get(base, '/v2/users/' + USER + '/rights', admin)['rights'] != expected:
        raise ValueError('Source permissions differ from the one-party read-only scope')
    user = get(base, '/v2/users/' + USER, admin)['user']
    if user.get('isDeactivated') or user.get('primaryParty') != party:
        raise ValueError('Source user is inactive or no longer matches the selected party')
    with urlopen('http://127.0.0.1:18080/readyz', timeout=10) as response:
        health = json.load(response)
    if health.get('status') != 'ok' or any(health.get(key) is not True for key in
                                         ('grpc_up', 'jdbc_connection_pool_up', 'stream_up')):
        raise ValueError('PQS source, database or stream is not ready')
    # SQL stdin keeps the party identifier out of process arguments and output.
    literal = "'" + party.replace("'", "''") + "'"
    sql = rf'''BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT set_latest({bound}) AS latest_offset \gset
SELECT set_oldest(oldest_offset()) AS oldest_offset \gset
SELECT json_build_object(
  'latest_offset', :latest_offset,
  'oldest_offset', :oldest_offset,
  'pruned_offset', pruned_offset(),
  'active_rows', (SELECT count(*) FROM active()),
  'create_rows', (SELECT count(*) FROM creates()),
  'duplicate_create_rows', (SELECT count(*) - count(DISTINCT (create_event_id, template_fqn, payload_type)) FROM creates()),
  'archive_rows', (SELECT count(*) FROM archives()),
  'exercise_rows', (SELECT count(*) FROM exercises()),
  'all_creates_in_scope', (SELECT bool_and({literal} = ANY(witnesses)) FROM creates())
);
COMMIT;
'''
    env = {**os.environ, 'PQS_UID': str(os.getuid()), 'PQS_GID': str(os.getgid())}
    result = subprocess.run([
        'docker', 'compose', '-p', 'cantongrid-pqs', '-f', str(ROOT / 'dev/pqs.compose.yaml'),
        'exec', '-T', '-e', 'PGPASSWORD=local-pqs-reader', 'pqs-postgres',
        'psql', '-X', '-qAt', '-h', '127.0.0.1', '-U', 'cantongrid_pqs_reader', '-d', 'pqs',
        '-v', 'ON_ERROR_STOP=1'], input=sql, text=True, capture_output=True, check=True, env=env)
    data = json.loads(result.stdout)
    if (data['create_rows'] < 1 or data['active_rows'] < 1
            or data['duplicate_create_rows'] != 0 or not data['all_creates_in_scope']):
        raise ValueError('Expected nonempty authorized contract data with matching witness evidence')
    print(json.dumps(data, indent=2))
    print('PASS: scoped source grants, pinned read-only SQL snapshot, contract and lifecycle queries')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'Local PQS probe failed: {exc}', file=sys.stderr)
        sys.exit(1)
