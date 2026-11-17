"""Shared definitions for the owned LocalNet visibility scenario, never runtime ingestion."""
import json
import os
from pathlib import Path
from urllib.error import HTTPError
from zipfile import ZipFile
from localnet_api import get, request, token

ROOT = Path(__file__).resolve().parents[1]
DIRECTORY = ROOT / '.local/visibility'
DAR = ROOT / 'examples/visibility/.daml/dist/cantongrid-visibility-fixture-0.1.0.dar'
BASE = 'http://127.0.0.1:3975'
TEMPLATE = '#cantongrid-visibility-fixture:Visibility:Record'
WRITER = 'cantongrid-visibility-writer'
USERS = {name: 'cantongrid-visibility-' + name for name in ('a', 'b', 'ab')}
AMOUNT = '12345678901234567890.1234567891'
SEQUENCE = '9223372036854775807'


def right(kind, party):
    return {'kind': {kind: {'value': {'party': party}}}}


def rights_equal(actual, expected):
    return sorted(json.dumps(r, sort_keys=True) for r in actual) == sorted(
        json.dumps(r, sort_keys=True) for r in expected)


def ensure_user(name, rights, primary=''):
    admin = token()
    try:
        user = get(BASE, '/v2/users/' + name, admin)['user']
    except HTTPError as exc:
        if exc.code != 404:
            raise
        user = request(BASE, '/v2/users', admin, body={
            'user': {'id': name, 'isDeactivated': False, 'primaryParty': primary,
                     'identityProviderId': '', 'metadata': {'annotations': {}}},
            'rights': rights})['user']
    actual = get(BASE, '/v2/users/' + name + '/rights', admin)['rights']
    if user.get('isDeactivated') or user.get('primaryParty', '') != primary or not rights_equal(actual, rights):
        raise ValueError('Owned fixture user does not match its expected scope; no rights were replaced')


def acs(subject, parties, bearer=None):
    offset = get(BASE, '/v2/state/ledger-end', token())['offset']
    selection = {'cumulative': [{'identifierFilter': {
        'TemplateFilter': {'value': {'templateId': TEMPLATE}}}}]}
    rows = request(BASE, '/v2/state/active-contracts?limit=100', bearer or token(subject), body={
        'activeAtOffset': offset,
        'eventFormat': {'filtersByParty': {p: selection for p in parties}, 'verbose': True}})
    events = []
    for row in rows:
        entry = row.get('contractEntry', {})
        if 'JsActiveContract' in entry:
            events.append(entry['JsActiveContract']['createdEvent'])
        elif entry and 'JsEmpty' not in entry:
            raise ValueError('Unexpected incomplete/reassignment entry in the controlled fixture')
    return events


def write_private(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, 'w') as output:
        output.write(json.dumps(value, indent=2) + '\n')


def package_id():
    with ZipFile(DAR) as archive:
        lines = []
        for line in archive.read('META-INF/MANIFEST.MF').decode().splitlines():
            if line.startswith(' '):
                lines[-1] += line[1:]
            else:
                lines.append(line)
    manifest = dict(line.split(': ', 1) for line in lines if ': ' in line)
    return Path(manifest['Main-Dalf']).stem.rsplit('-', 1)[1]


def context():
    data = json.loads((DIRECTORY / 'context.json').read_text())
    if data['packageId'] != package_id():
        raise ValueError('Built fixture package differs from saved scope; review before reusing it')
    return data
