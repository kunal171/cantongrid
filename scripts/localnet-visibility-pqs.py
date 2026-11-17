#!/usr/bin/env python3
"""Run the AB-scoped source experiment in a separate PQS database."""
import argparse
import os
import subprocess
import sys
from localnet_api import token
from localnet_visibility import DIRECTORY, ROOT, USERS, context, ensure_user, right, write_private

COMPOSE = ['docker', 'compose', '--progress', 'plain', '-p', 'cantongrid-visibility',
           '-f', str(ROOT / 'dev/visibility.compose.yaml')]
ENV = {**os.environ, 'PQS_UID': str(os.getuid()), 'PQS_GID': str(os.getgid())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['up', 'stop', 'status'])
    args = parser.parse_args()
    if args.command == 'up':
        data = context()
        ensure_user(USERS['ab'], [right('CanReadAs', p) for p in data['parties'].values()])
        write_private(DIRECTORY / 'source.conf', {
            'source': {'ledger': {'host': 'canton', 'port': 3901, 'auth': 'OAuth'}},
            'pipeline': {'oauth': {'accessToken': token(USERS['ab'], 3600)},
                         'filter': {'parties': '*', 'contracts': '*', 'metadata': '!*'},
                         'ledger': {'start': 0}},
            'target': {'postgres': {'host': 'pqs-postgres', 'port': 5432, 'database': 'cantongrid_visibility',
                                    'username': 'pqs_owner', 'password': 'local-pqs-only',
                                    'schema': 'public'},
                       'encoding': {'numericAsString': True, 'int64AsString': True}},
            'health': {'address': '0.0.0.0', 'port': 8080}, 'logger': {'level': 'Warning'}})
        database = ['docker', 'compose', '-p', 'cantongrid-pqs',
                    '-f', str(ROOT / 'dev/pqs.compose.yaml'), 'exec', '-T', 'pqs-postgres',
                    'psql', '-X', '-q', '-U', 'pqs_owner', '-v', 'ON_ERROR_STOP=1']
        subprocess.run([*database, '-d', 'pqs'], check=True, text=True, input=r"""
SELECT 'CREATE DATABASE cantongrid_visibility OWNER pqs_owner'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'cantongrid_visibility')\gexec
""")
        subprocess.run([*database, '-d', 'cantongrid_visibility'], check=True, text=True, input=r"""
REVOKE CONNECT, TEMPORARY ON DATABASE cantongrid_visibility FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
""")
        command = ['up', '-d', '--force-recreate']
    else:
        command = ['down'] if args.command == 'stop' else ['ps', '-a']
    subprocess.run([*COMPOSE, *command], check=True, env=ENV)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as exc:
        print(f'Visibility PQS setup failed: {exc}', file=sys.stderr)
        sys.exit(1)
