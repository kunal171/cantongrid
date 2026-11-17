#!/usr/bin/env python3
"""Provision a narrowly scoped local PQS compatibility fixture."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
from urllib.error import HTTPError
from localnet_api import get, request, token

ROOT = Path(__file__).resolve().parents[1]
COMPOSE = ['docker', 'compose', '--progress', 'plain', '-p', 'cantongrid-pqs',
           '-f', str(ROOT / 'dev/pqs.compose.yaml')]
USER = 'cantongrid-pqs-app-provider'
BASE = 'http://127.0.0.1:3975'
ENV = {**os.environ, 'PQS_UID': str(os.getuid()), 'PQS_GID': str(os.getgid())}


def prepare():
    admin = token()
    party = get(BASE, '/v2/users/ledger-api-user', admin)['user'].get('primaryParty')
    if not party:
        raise ValueError('App Provider has no verified primary party; refusing to guess the source scope')
    expected = [{'kind': {'CanReadAs': {'value': {'party': party}}}}]
    path = '/v2/users/' + USER
    try:
        user = get(BASE, path, admin)['user']
    except HTTPError as exc:
        if exc.code != 404:
            raise
        user = request(BASE, '/v2/users', admin, body={
            'user': {'id': USER, 'isDeactivated': False, 'primaryParty': party,
                     'identityProviderId': '', 'metadata': {'annotations': {}}},
            'rights': expected})['user']
    actual = get(BASE, path + '/rights', admin)['rights']
    if user.get('isDeactivated') or user.get('primaryParty') != party or actual != expected:
        raise ValueError('Existing PQS user differs from the exact read-only scope; no permissions were changed')
    values = {
        'source.ledger.host': 'canton', 'source.ledger.port': 3901,
        'source.ledger.auth': 'OAuth', 'pipeline.oauth.accessToken': token(USER, 3600),
        'pipeline.filter.parties': party, 'pipeline.filter.contracts': '*',
        'pipeline.filter.metadata': '*', 'pipeline.ledger.start': 0,
        'target.postgres.host': 'pqs-postgres', 'target.postgres.port': 5432,
        'target.postgres.database': 'pqs', 'target.postgres.username': 'pqs_owner',
        'target.postgres.password': 'local-pqs-only',
        'target.encoding.numericAsString': True, 'target.encoding.int64AsString': True,
        'health.address': '0.0.0.0', 'health.port': 8080, 'logger.level': 'Warning',
    }
    directory = ROOT / '.local/pqs'
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    config = directory / 'source.conf'
    # Open owner-only from creation; never print the signed token or party ID.
    fd = os.open(config, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, 'w') as output:
        output.write('\n'.join(f'{key}={json.dumps(value)}' for key, value in values.items()) + '\n')
    print('Prepared one-party CanReadAs source user; local token expires in one hour')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['up', 'stop', 'status'])
    args = parser.parse_args()
    if args.command == 'up':
        prepare()
        subprocess.run([*COMPOSE, 'up', '-d', '--wait', 'pqs-postgres'], check=True, env=ENV)
        # Recreate the process so it loads the freshly signed credential.
        subprocess.run([*COMPOSE, 'up', '-d', '--force-recreate', '--no-deps', 'pqs'], check=True, env=ENV)
    else:
        command = ['down'] if args.command == 'stop' else ['ps', '-a']
        subprocess.run([*COMPOSE, *command], check=True, env=ENV)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'Local PQS fixture failed: {exc}', file=sys.stderr)
        sys.exit(1)
