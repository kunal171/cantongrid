"""LocalNet acceptance for generated model queries; never a product query processor."""
from contextlib import contextmanager
from dataclasses import dataclass
from decimal import Decimal
import json
import subprocess
from urllib.error import HTTPError

import lifecycle
import support as s


@dataclass(frozen=True)
class Case:
    """Independent expected values and visibility for one real Daml test contract."""
    name: str
    owner: str
    readers: tuple[str, ...]
    price: str
    quantity: str
    outcome: bool | None


# 2 < 10 numerically, while lexicographic sorting would reverse them. Quantities exceed 2**53.
CASES = (
    Case('a_low', 'a', (), '2.0000000000', '9007199254740993', None),
    Case('joint_ten', 'a', ('b',), '10.0000000000', '9007199254740994', None),
    Case('b_private', 'b', (), '1.0000000000', '9223372036854775807', False),
    Case('joint_true', 'a', ('b',), '10.0000000000', '9007199254740995', True),
)


def seed():
    """Called only by the explicitly authorized run command; reuse saved fixture identities."""
    for case in CASES:
        data = s.state()
        if case.name in data.get('queryContracts', {}):
            continue
        cid = lifecycle.submit({'CreateCommand': {
            'templateId': s.TEMPLATE,
            'createArguments': {
                'operator': data['parties'][case.owner],
                'readers': [data['parties'][role] for role in case.readers],
                'demoId': 'query-' + data['source'] + '-' + case.name,
                'question': data['source'] + '-query',
                'priceTick': case.price, 'quantityLot': case.quantity,
                'state': 'Draft' if case.outcome is None else 'Resolved',
                'outcome': case.outcome,
            }}}, 'query-' + case.name)
        data = s.state()
        data.setdefault('queryContracts', {})[case.name] = cid
        s.save(data)


def filters():
    """Select only this run's query controls, excluding the original lifecycle and other runs."""
    return {'question': {'eq': s.state()['source'] + '-query'}}


def endpoint():
    return '/v1/models/' + s.state()['namespace'] + '/markets'


def metadata():
    """Require discovery parity without exposing definition expressions or private literal values."""
    data = s.state()
    listing = s.api('ab', '/v1/models')['models']
    entry = next(row for row in listing if row['namespace'] == data['namespace'] and row['name'] == 'markets')
    described = s.api('ab', endpoint())
    gql = s.api('ab', '/models/graphql', {'query': '''{
      models {
        namespace name revision fingerprint queryField
        fields { name kind nullable scale operators }
      }
    }'''})
    if gql.get('errors') or gql['data']['models'] != listing or described != entry:
        raise ValueError('REST/GraphQL model discovery differs')
    if set(entry) != {'namespace', 'name', 'revision', 'fingerprint', 'queryField', 'fields'}:
        raise ValueError('Unexpected model discovery metadata')
    for field in entry['fields']:
        if set(field) != {'name', 'kind', 'nullable', 'scale', 'operators'}:
            raise ValueError('Discovery exposed unsupported field metadata')
    return entry


def rejected(call, *, graphql=False, status=400):
    """A source outage is not evidence that input validation rejected a bad cursor."""
    try:
        call()
    except HTTPError as error:
        if error.code != status:
            raise ValueError('Unexpected rejection status in model query control') from None
    except ValueError:
        if not graphql:
            raise
    else:
        raise ValueError('Invalid or unauthorized model query was accepted')


def check_rows(principal, transport, expected, **options):
    """Compare every one-row page, its filtered count and exact typed values with ledger controls."""
    data = s.state()
    items = []
    cursor = None
    for _ in range(len(CASES) + 1):
        page = s.query(principal, transport=transport, limit=1, cursor=cursor, **options)
        if page['visibleTotal'] != str(len(expected)):
            raise ValueError('Filtered count included hidden contracts or ignored a predicate')
        items.extend(page['items'])
        cursor = page['nextCursor']
        if not cursor:
            break
    else:
        raise ValueError('Query cursor repeated or exceeded its expected page budget')
    if [row['contractId'] for row in items] != [data['queryContracts'][case.name] for case in expected]:
        raise ValueError('Model query visibility, sorting or page boundary differs')
    for row, case in zip(items, expected):
        values = row['values']
        if (values['priceTick'] != case.price or values['quantityLot'] != case.quantity
                or values['outcome'] is not case.outcome
                or Decimal(values['priceTickBps']) != Decimal(case.price) * 10000):
            raise ValueError('Model query changed an exact value or optional boolean')


@contextmanager
def promoted():
    """Always restore revision 1, including after assertions or an ambiguous promotion response."""
    data = s.state()
    definition = json.loads((s.DIRECTORY / 'models.json').read_text())
    definition['models'][0]['revision'] = 2
    path = s.DIRECTORY / 'query-revision2.json'
    s.write_private(path, definition)
    s.cli('models', 'register', path, '--namespace', data['namespace'], '--database-env', 'CANTONGRID_DESTINATION_URL')
    s.cli('models', 'install', data['namespace'], 'markets', 2, '--database-env', 'CANTONGRID_DESTINATION_URL')
    try:
        s.cli('models', 'bind', s.DIRECTORY / 'runtime.json', data['namespace'], 'markets', 2, '--replace', 1)
        yield
    finally:
        # bind is compare-and-replace and an idempotent retry if revision 1 never changed.
        s.cli('models', 'bind', s.DIRECTORY / 'runtime.json', data['namespace'], 'markets', 1, '--replace', 2)


def source_outage():
    """Retained rows must not be served when the authorized exporter is unavailable."""
    from devtools.common import wait_source
    before = s.metrics()
    try:
        subprocess.run([*s.compose(True), 'stop', 'pqs-ab'], env=s.ENV, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for transport in ('rest', 'graphql'):
            rejected(lambda: s.query(transport=transport, filters=filters()), status=503)
        rejected(lambda: s.api('ab', '/v1/models'), status=503)
    finally:
        # The source was healthy at entry; restore it even if stop or an assertion failed.
        subprocess.run([*s.compose(True), 'start', 'pqs-ab'], env=s.ENV, check=True,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        wait_source(True)
    if s.metrics() != before or s.query(transport='rest', filters=filters())['visibleTotal'] != str(len(CASES)):
        raise ValueError('Exporter recovery changed durable progress or lost model access')


def verify(*, revisions=False, outage=False):
    """Exercise the unchanged Rust APIs against real source-backed contracts, never synthetic SQL rows."""
    data = s.state()
    before = metadata()
    base = filters()
    page_checks = 0
    for principal in ('a', 'b', 'ab'):
        visible = [case for case in CASES if principal == 'ab' or principal in (case.owner, *case.readers)]
        by_id = lambda case: data['queryContracts'][case.name]
        for transport in ('graphql', 'rest'):
            for direction in ('asc', 'desc'):
                # Equal prices always use ascending IDs, including descending value order.
                ordered = sorted(visible, key=lambda c: ((1 if direction == 'asc' else -1) * Decimal(c.price), by_id(c)))
                check_rows(principal, transport, ordered, filters=base, sort={'field': 'priceTick', 'direction': direction})
                page_checks += 1
            for predicate, expected in [
                ({'priceTick': {'eq': '10'}}, [c for c in visible if c.price == '10.0000000000']),
                ({'quantityLot': {'gte': '9007199254740994', 'lt': '9223372036854775807'}}, [c for c in visible if c.name in ('joint_ten', 'joint_true')]),
                ({'outcome': {'isNull': True}}, [c for c in visible if c.outcome is None]),
                ({'outcome': {'eq': False}}, [c for c in visible if c.outcome is False]),
                ({'outcome': {'ne': False}}, [c for c in visible if c.outcome is True]),
            ]:
                check_rows(principal, transport, sorted(expected, key=by_id), filters={**base, **predicate})
                page_checks += 1
            for direction in ('asc', 'desc'):
                ordered = sorted(visible, key=lambda c: (c.outcome is None, (1 if direction == 'asc' else -1) * int(c.outcome or False), by_id(c)))
                check_rows(principal, transport, ordered, filters=base, sort={'field': 'outcome', 'direction': direction})
                page_checks += 1
    options = {'filters': {**base, 'priceTick': {'gte': '1'}}, 'sort': {'field': 'priceTick'}}
    cursor = s.query('ab', transport='rest', limit=1, **options)['nextCursor']
    normalized = {**options, 'filters': {**base, 'priceTick': {'gte': '1.0000000000'}}}
    if not cursor or s.query('ab', cursor=cursor, **normalized)['items'] != s.query('ab', transport='rest', cursor=cursor, **options)['items']:
        raise ValueError('Equivalent REST/GraphQL queries could not share their cursor')
    for transport in ('rest', 'graphql'):
        for args in [
            {'subject': 'a', **options},
            {'subject': 'ab', **options, 'sort': {'field': 'priceTick', 'direction': 'desc'}},
            {'subject': 'ab', **options, 'filters': {**base, 'priceTick': {'gte': '2'}}},
        ]:
            rejected(lambda: s.query(transport=transport, cursor=cursor, **args), graphql=transport == 'graphql')
        rejected(lambda: s.query(transport=transport, cursor=cursor + 'x', **options), graphql=transport == 'graphql')
        for invalid in [
            {'filters': {'priceTick': {'eq': '1.00000000001'}}},
            {'filters': {'quantityLot': {'eq': '9223372036854775808'}}},
            {'filters': {'outcome': {'eq': None}}},
            {'sort': {'field': 'priceTick; DROP TABLE x'}},
        ]:
            rejected(lambda: s.query(transport=transport, **invalid), graphql=transport == 'graphql')
    grants_path = s.DIRECTORY / 'grants.json'
    grants = json.loads(grants_path.read_text())
    try:
        changed = {**grants, 'epoch': grants['epoch'] + 1}
        s.write_private(grants_path, changed)
        rejected(lambda: s.query(transport='rest', cursor=cursor, **options))
    finally:
        # Keep epochs monotonic; do not revive cursors issued before the test update.
        s.write_private(grants_path, {**grants, 'epoch': grants['epoch'] + 2})
    if revisions:
        cursor = s.query(transport='rest', limit=1, **options)['nextCursor']
        with promoted():
            active = metadata()
            if active['revision'] != '2' or active['fingerprint'] == before['fingerprint']:
                raise ValueError('Promoted revision was not served by discovery')
            rejected(lambda: s.query(transport='rest', cursor=cursor, **options))
            if s.query(transport='rest', filters=base)['visibleTotal'] != str(len(CASES)):
                raise ValueError('Promoted projection lost current authorized contracts')
        if metadata() != before:
            raise ValueError('Original model revision was not restored')
    if outage:
        source_outage()
    return {'filteredPageSequences': page_checks, 'modelDiscoveryParity': True,
            'modelRestGraphqlParity': True, 'numericAndNullableSorting': True,
            'queryCursorBinding': True, 'grantEpochCursorInvalidation': True,
            'revisionPromotionRollback': revisions, 'modelSourceOutageRecovery': outage}
