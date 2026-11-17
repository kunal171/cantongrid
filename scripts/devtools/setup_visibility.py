"""Deploy only the owned visibility test package/parties/records to LocalNet."""
from .common import write_private
import json
from urllib.error import HTTPError
from .ledger import get, request, token
from .visibility import (
    AMOUNT, BASE, DAR, DIRECTORY, SEQUENCE, TEMPLATE, USERS, WRITER,
    acs, ensure_user, package_id, right,
)


def main():
    pid = package_id()
    saved = DIRECTORY / 'context.json'
    if saved.exists() and json.loads(saved.read_text())['packageId'] != pid:
        raise ValueError('Existing fixture uses another package; refusing to overwrite its scope')
    admin = token()
    if pid not in get(BASE, '/v2/packages', admin)['packageIds']:
        request(BASE, '/v2/dars/validate', admin, body=DAR.read_bytes(),
                content_type='application/octet-stream', read_json=False, timeout=120)
        request(BASE, '/v2/dars?vetAllPackages=true', admin, body=DAR.read_bytes(),
                content_type='application/octet-stream', read_json=False, timeout=120)
    try:
        get(BASE, '/v2/users/' + WRITER, admin)
    except HTTPError as exc:
        if exc.code != 404:
            raise
        ensure_user(WRITER, [])
    parties = {}
    for name in ('a', 'b'):
        hint = 'cantongrid-visibility-' + name
        known = [row['party'] for row in get(BASE, '/v2/parties', admin)['partyDetails']
                 if row.get('isLocal') and row['party'].startswith(hint + '::')]
        if len(known) > 1:
            raise ValueError('Fixture party hint is ambiguous')
        parties[name] = known[0] if known else request(BASE, '/v2/parties', admin, body={
            'partyIdHint': hint, 'identityProviderId': '', 'userId': WRITER})['partyDetails']['party']
    ensure_user(WRITER, [right('CanActAs', p) for p in parties.values()])
    for name in USERS:
        selected = list(parties.values()) if name == 'ab' else [parties[name]]
        ensure_user(USERS[name], [right('CanReadAs', p) for p in selected])
    rows = acs(WRITER, list(parties.values()))
    if not rows:
        commands = []
        for label, owner, readers in [('a-only', parties['a'], []), ('b-only', parties['b'], []),
                                       ('joint', parties['a'], [parties['b']])]:
            commands.append({'CreateCommand': {'templateId': TEMPLATE, 'createArguments': {
                'owner': owner, 'readers': readers, 'label': label, 'amount': AMOUNT,
                'sequence': SEQUENCE, 'note': None, 'state': 'Ready'}}})
        request(BASE, '/v2/commands/submit-and-wait-for-transaction', token(WRITER), body={
            'commands': {'commandId': 'cantongrid-visibility-seed-v1', 'userId': WRITER,
                         'actAs': list(parties.values()), 'commands': commands}}, timeout=120)
        rows = acs(WRITER, list(parties.values()))
    if len(rows) != 3 or {r['createArgument']['label'] for r in rows} != {'a-only', 'b-only', 'joint'}:
        raise ValueError('Fixture records differ from the three controlled labels; no replacement was attempted')
    write_private(saved, {'packageId': pid, 'parties': parties})
    print('PASS: owned fixture package, two test parties, writer and three read-only scopes are ready')
