"""Explicitly scoped local PQS setup; shared lifecycle with separate source profiles."""
from .common import write_private
import argparse
import subprocess
from urllib.error import HTTPError
from .common import ENV, ROOT, compose, pg_command, write_private, wait_source
from .ledger import get, request, token
from .visibility import DIRECTORY, USERS, context, ensure_user, right

USER = 'cantongrid-pqs-app-provider'
BASE = 'http://127.0.0.1:3975'


def source_config(subject, parties, metadata, database):
    # JSON objects must be nested: quoted dotted keys would be literal HOCON keys.
    return {
        'source': {'ledger': {'host': 'canton', 'port': 3901, 'auth': 'OAuth'}},
        'pipeline': {'oauth': {'accessToken': token(subject, 3600)},
                     'filter': {'parties': parties, 'contracts': '*', 'metadata': metadata},
                     'ledger': {'start': 0}},
        'target': {'postgres': {'host': 'pqs-postgres', 'port': 5432, 'database': database,
                                'username': 'pqs_owner', 'password': 'local-pqs-only', 'schema': 'public'},
                   'encoding': {'numericAsString': True, 'int64AsString': True}},
        'health': {'address': '0.0.0.0', 'port': 8080}, 'logger': {'level': 'Warning'},
    }


def prepare_provider():
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
    write_private(ROOT / '.local/pqs/source.conf', source_config(USER, party, '*', 'pqs'))
    print('Prepared one-party CanReadAs source user; local token expires in one hour')


def prepare_visibility():
    data = context()
    ensure_user(USERS['ab'], [right('CanReadAs', p) for p in data['parties'].values()])
    write_private(DIRECTORY / 'source.conf', source_config(USERS['ab'], '*', '!*', 'cantongrid_visibility'))
    database = pg_command(tuples=False)
    subprocess.run(database, check=True, text=True, env=ENV, input=r"""
SELECT 'CREATE DATABASE cantongrid_visibility OWNER pqs_owner'
WHERE NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = 'cantongrid_visibility')\gexec
""")
    subprocess.run(pg_command('cantongrid_visibility', tuples=False), check=True, text=True, env=ENV, input=r"""
REVOKE CONNECT, TEMPORARY ON DATABASE cantongrid_visibility FROM PUBLIC;
REVOKE CREATE ON SCHEMA public FROM PUBLIC;
""")


def up(*, visibility=False, quiet=False):
    if visibility:
        prepare_visibility()
    else:
        prepare_provider()
        subprocess.run([*compose(), 'up', '-d', '--wait', 'pqs-postgres'], check=True, env=ENV)
    command = [*compose(visibility), 'up', '-d', '--force-recreate']
    if not visibility:
        command += ['--no-deps', 'pqs']
    subprocess.run(command, check=True, env=ENV,
                   stdout=subprocess.DEVNULL if quiet else None)
    wait_source(visibility)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['up', 'stop', 'status', 'reader'])
    parser.add_argument('--visibility', action='store_true', help='Use the isolated AB visibility source')
    args = parser.parse_args(argv)
    if args.command == 'up':
        up(visibility=args.visibility)
    elif args.command == 'reader':
        sql = (ROOT / 'dev/pqs-reader.sql').read_text()
        if args.visibility:
            # Separate role/database: never widen the provider reader's party scope.
            sql = sql.replace("current_database() <> 'pqs'", "current_database() <> 'cantongrid_visibility'")
            sql = sql.replace('DATABASE pqs ', 'DATABASE cantongrid_visibility ')
            sql = sql.replace('cantongrid_pqs_reader', 'cantongrid_visibility_reader')
            sql = sql.replace('local-pqs-reader', 'local-visibility-reader')
        subprocess.run(pg_command('cantongrid_visibility' if args.visibility else 'pqs', tuples=False),
                       input=sql, text=True, check=True, env=ENV)
        print('Prepared development SQL reader; source ownership remains with PQS')
    else:
        command = ['down'] if args.command == 'stop' else ['ps', '-a']
        subprocess.run([*compose(args.visibility), *command], check=True, env=ENV)
