"""Exercise A/B/AB reads and reversible revocation on the owned test users only."""
from urllib.error import HTTPError
from ..ledger import get, request, token
from ..visibility import (
    AMOUNT, BASE, SEQUENCE, USERS, acs, context, ensure_user, right, rights_equal,
)


def check_rows(events, expected):
    """Check expected fixture labels and exact values rather than relying on row counts alone."""
    if len(events) != len(expected) or {r['createArgument']['label'] for r in events} != expected:
        raise ValueError('Ledger visibility returned unexpected fixture records')
    for event in events:
        value = event['createArgument']
        if (value['amount'] != AMOUNT or value['sequence'] != SEQUENCE
                or value['note'] is not None or value['state'] != 'Ready'):
            raise ValueError('Exact Numeric/Int64, optional or enum round trip changed')


def expect_forbidden(subject, parties, bearer=None):
    """Require a permission denial for a party selection outside the verified source scope."""
    try:
        acs(subject, parties, bearer)
    except HTTPError as exc:
        if exc.code == 403:
            return
        raise
    raise ValueError('Unauthorized party filter was accepted')


def main():
    """Exercise owned A/B/AB reads, then revoke and restore AB rights in a finally block."""
    data = context()
    parties = data['parties']
    expected = {'a': {'a-only', 'joint'}, 'b': {'b-only', 'joint'},
                'ab': {'a-only', 'b-only', 'joint'}}
    for name in USERS:
        selected = list(parties.values()) if name == 'ab' else [parties[name]]
        ensure_user(USERS[name], [right('CanReadAs', p) for p in selected])
        check_rows(acs(USERS[name], selected), expected[name])
        print(f'PASS: {name.upper()} scope sees its {len(expected[name])} expected records with exact values')
    expect_forbidden(USERS['a'], [parties['b']])
    expect_forbidden(USERS['b'], [parties['a']])
    expect_forbidden(USERS['a'], list(parties.values()))
    print('PASS: cross-party and combined filters cannot widen a one-party grant')

    stale = token(USERS['ab'])
    revoked = right('CanReadAs', parties['b'])
    path = '/v2/users/' + USERS['ab'] + '/rights'
    change = {'userId': USERS['ab'], 'identityProviderId': '', 'rights': [revoked]}
    try:
        request(BASE, path, token(), method='PATCH', body=change)
        self_rights = get(BASE, path, stale)['rights']
        if not rights_equal(self_rights, [right('CanReadAs', parties['a'])]):
            raise ValueError('Revoked source cannot read its own current grant set')
        expect_forbidden(USERS['ab'], [parties['b']], stale)
        expect_forbidden(USERS['ab'], list(parties.values()), stale)
        expect_forbidden(USERS['ab'], [parties['b']])
        check_rows(acs(USERS['ab'], [parties['a']], stale), expected['a'])
        check_rows(acs(USERS['b'], [parties['b']]), expected['b'])
        print('PASS: revoked B reads fail with both preissued and fresh tokens; A and the independent B user still work')
    finally:
        request(BASE, path, token(), body=change)
    ensure_user(USERS['ab'], [right('CanReadAs', p) for p in parties.values()])
    check_rows(acs(USERS['ab'], list(parties.values())), expected['ab'])
    print('PASS: AB permissions restored and original visibility recovered')
    print('No Outcomia contracts or wallet balances were modified')
