"""Real Outcomia model acceptance; application fixtures stay outside the Rust indexing engine."""
from contextlib import contextmanager
from dataclasses import dataclass
from decimal import Decimal
import json
from pathlib import Path
from typing import Callable
import subprocess
import time
from urllib.error import HTTPError

from ..common import ROOT, write_private


@dataclass(frozen=True)
class Market:
    """Actual created contract identity with independent expected values and party visibility."""
    contract: str
    parties: frozenset[str]
    quantity: str
    price: str
    status: str
    reason: str | None


def created(events, package, entity):
    """Pin transaction results to the inspected DAR, rather than selecting an arbitrary active row."""
    matches = [event for event in events if event['templateId'] == f'{package}:Outcomia.Binary:{entity}']
    if len(matches) != 1:
        raise ValueError('Expected exactly one Outcomia contract of the pinned template')
    return matches[0]['contractId']


@dataclass(repr=False)
class Probe:
    """Owned application-test context; repr excludes the child environment and API credentials."""
    binary: Path
    directory: Path
    env: dict
    query: Callable
    namespace: str
    selected: dict | None = None

    def cli(self, *args):
        result = subprocess.run([str(self.binary), *map(str, args)], env=self.env,
                                capture_output=True, text=True)
        if result.returncode:
            raise ValueError('Outcomia model check failed: ' + ' '.join(map(str, args[:2])))
        return result.stdout

    def metadata(self):
        """Discover this active binding and compare REST and GraphQL schema metadata."""
        listing = self.query('/v1/models', 'ab')['models']
        entry = next(row for row in listing if row['namespace'] == self.namespace and row['name'] == 'markets')
        described = self.query(self.path(), 'ab')
        gql = self.query('/models/graphql', 'ab', {'query': '''{
          models {
            namespace name revision fingerprint queryField
            fields { name kind nullable scale operators }
          }
        }'''})
        if gql.get('errors') or gql['data']['models'] != listing or described != entry:
            raise ValueError('Outcomia model discovery differs between transports')
        if set(entry) != {'namespace', 'name', 'revision', 'fingerprint', 'queryField', 'fields'}:
            raise ValueError('Unexpected Outcomia model metadata')
        if any(set(field) != {'name', 'kind', 'nullable', 'scale', 'operators'} for field in entry['fields']):
            raise ValueError('Unexpected Outcomia field metadata')
        self.selected = entry
        return entry

    def path(self):
        return '/v1/models/' + self.namespace + '/markets'

    def read(self, principal='ab', transport='rest', **options):
        """Use the generated schema and exact string arguments; never process indexed rows here."""
        body = {'limit': 1, **options}
        if transport == 'rest':
            return self.query(self.path() + '/query', principal, body)
        stem = 'Model_' + self.selected['fingerprint'][:16]
        field = self.selected['queryField']
        values = ' '.join(column['name'] for column in self.selected['fields'])
        result = self.query('/models/graphql', principal, {
            'query': f'''query($namespace:String!,$limit:Int,$cursor:String,$filters:{stem}Filter,$sort:{stem}Sort) {{
              {field}(namespace:$namespace,limit:$limit,cursor:$cursor,filters:$filters,sort:$sort) {{
                visibleTotal nextCursor fingerprint
                items {{ contractId values {{ {values} }} }}
              }}
            }}''', 'variables': {'namespace': self.namespace, **body}})
        if result.get('errors'):
            raise ValueError('Outcomia generated GraphQL query rejected')
        return result['data'][field]

    def pages(self, principal, transport, expected, **options):
        """Check every count and keyset boundary against actual Daml submission identities."""
        cursor = None
        rows = []
        for _ in range(4):
            page = self.read(principal, transport, cursor=cursor, **options)
            if page['visibleTotal'] != str(len(expected)):
                raise ValueError('Outcomia filtered count exposed hidden data or ignored a filter')
            rows.extend(page['items'])
            cursor = page['nextCursor']
            if not cursor:
                break
        else:
            raise ValueError('Outcomia keyset pagination repeated or exceeded its budget')
        if [row['contractId'] for row in rows] != [case.contract for case in expected]:
            raise ValueError('Outcomia projected contract identities/order differ from ledger results')
        for row, case in zip(rows, expected):
            values = row['values']
            if (values['quantityLot'] != case.quantity or values['priceTick'] != case.price
                    or values['status'] != case.status
                    or values['priceTickBps'] != format(Decimal(case.price) * 10000, '.10f')):
                raise ValueError('Outcomia model changed exact or lifecycle values')
            if self.selected['revision'] == '2' and values['closeReason'] != case.reason:
                raise ValueError('Outcomia optional closure reason differs from the actual contract')

    @contextmanager
    def promoted(self):
        """Introduce a real optional output field, restoring revision 1 even after failed assertions."""
        definition = json.loads((self.directory / 'models.json').read_text())
        definition['models'][0]['revision'] = 2
        definition['models'][0]['fields']['closeReason'] = {
            'op': 'field', 'path': ['closeReason'], 'as': {'type': 'optional', 'item': {'type': 'text'}}}
        path = self.directory / 'models-v2.json'
        write_private(path, definition)
        self.cli('models', 'register', path, '--namespace', self.namespace, '--database-env', 'CANTONGRID_DESTINATION_URL')
        self.cli('models', 'install', self.namespace, 'markets', 2, '--database-env', 'CANTONGRID_DESTINATION_URL')
        try:
            self.cli('models', 'bind', self.directory / 'runtime.json', self.namespace, 'markets', 2, '--replace', 1)
            self.metadata()
            yield
        finally:
            self.cli('models', 'bind', self.directory / 'runtime.json', self.namespace, 'markets', 1, '--replace', 2)
            self.metadata()


def rejected(call, *, graphql=False, status=400):
    """Only the expected input/auth error counts as a rejection; an outage is a failed control."""
    try:
        call()
    except HTTPError as error:
        if error.code != status:
            raise ValueError('Unexpected Outcomia query rejection status') from None
    except ValueError:
        if not graphql:
            raise
    else:
        raise ValueError('An invalid Outcomia model query/cursor was accepted')


def seed(*, submit, exercise, package, parties, identifier, terms, protocol):
    """Create A-only, B-only and shared markets through real protocol choices, without assets."""
    result = []
    marker = identifier + '-queries'
    for role, quantity, price in [('a', '2', '0.0200000000'), ('b', '10', '0.1000000000'), ('ab', '9007199254740993', '0.1000000000')]:
        if role != 'ab':
            events = submit({'CreateCommand': {'templateId': '#outcomia-markets:Outcomia.Binary:Protocol', 'createArguments': {
                'operator': parties[role], 'protocolName': identifier + '-' + role, 'admins': [parties[role]],
                'nextMarketId': '1', 'takerFeeBps': '50', 'feeRevision': '0'}}}, '-query-protocol-' + role)
            current = created(events, package, 'Protocol')
        else:
            current = protocol
        creator = parties['b' if role == 'ab' else role]
        options = {**terms, 'question': marker, 'quantityLot': quantity, 'priceTick': price}
        events = exercise('#outcomia-markets:Outcomia.Binary:Protocol', current, 'Protocol_CreateBinary',
                          {'creator': creator, 'terms': options}, '-query-create-' + role)
        current = created(events, package, 'Protocol')
        market = created(events, package, 'BinaryMarket')
        status, reason = 'Draft', None
        if role in ('a', 'ab'):
            events = exercise('#outcomia-markets:Outcomia.Binary:BinaryMarket', market, 'Binary_Open',
                              {'protocol': current}, '-query-open-' + role)
            market = created(events, package, 'BinaryMarket')
            status = 'Open'
        if role == 'a':
            reason = 'Local query acceptance complete'
            events = exercise('#outcomia-markets:Outcomia.Binary:BinaryMarket', market, 'Binary_Close',
                              {'protocol': current, 'reason': reason}, '-query-close-a')
            market = created(events, package, 'BinaryMarket')
            status = 'Closed'
        result.append(Market(market, frozenset(('a', 'b') if role == 'ab' else (role,)), quantity, price, status, reason))
    return result, {'question': {'eq': marker}}


def verify_models(*, binary, directory, env, query, package, parties, identifier, terms, protocol, submit, exercise):
    """Bind the application mapping, index real changes and exercise both public model transports."""
    probe = Probe(binary, directory, env, query, identifier.replace('-', '_'))
    definition = json.loads((ROOT / 'examples/models/outcomia.json').read_text())
    definition['models'][0]['template']['package_id'] = package
    write_private(directory / 'models.json', definition)
    probe.cli('models', 'register', directory / 'models.json', '--namespace', probe.namespace, '--database-env', 'CANTONGRID_DESTINATION_URL')
    probe.cli('models', 'install', probe.namespace, 'markets', 1, '--database-env', 'CANTONGRID_DESTINATION_URL')
    probe.cli('models', 'bind', directory / 'runtime.json', probe.namespace, 'markets', 1)
    original = probe.metadata()
    worker = subprocess.Popen([str(binary), 'index', str(directory / 'runtime.json')], env=env,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        cases, base = seed(submit=submit, exercise=exercise, package=package, parties=parties,
                           identifier=identifier, terms=terms, protocol=protocol)
        deadline = time.monotonic() + 60
        expected = {case.contract for case in cases}
        while True:
            page = probe.read(limit=25, filters=base)
            if {row['contractId'] for row in page['items']} == expected:
                break
            if worker.poll() is not None or time.monotonic() > deadline:
                raise ValueError('Live Outcomia model projection did not catch up')
            time.sleep(0.3)
        sequences = 0
        for principal in ('a', 'b', 'ab'):
            visible = [case for case in cases if principal == 'ab' or principal in case.parties]
            for transport in ('rest', 'graphql'):
                for direction in ('asc', 'desc'):
                    ordered = sorted(visible, key=lambda c: (int(c.quantity) * (1 if direction == 'asc' else -1), c.contract))
                    probe.pages(principal, transport, ordered, filters=base, sort={'field': 'quantityLot', 'direction': direction})
                    sequences += 1
                for predicate, expected_cases in [
                    ({'quantityLot': {'gte': '3'}}, [case for case in visible if int(case.quantity) >= 3]),
                    ({'priceTick': {'eq': '0.10'}}, [case for case in visible if Decimal(case.price) == Decimal('.1')]),
                    ({'priceTickBps': {'eq': '1000'}}, [case for case in visible if Decimal(case.price) == Decimal('.1')]),
                    ({'status': {'eq': 'Closed'}}, [case for case in visible if case.status == 'Closed']),
                ]:
                    probe.pages(principal, transport, sorted(expected_cases, key=lambda c: c.contract), filters={**base, **predicate})
                    sequences += 1
        options = {'filters': base, 'sort': {'field': 'quantityLot'}}
        cursor = probe.read(**options)['nextCursor']
        if not cursor or probe.read('ab', 'graphql', cursor=cursor, **options)['items'] != probe.read(cursor=cursor, **options)['items']:
            raise ValueError('Outcomia transport cursor parity failed')
        for transport in ('rest', 'graphql'):
            rejected(lambda: probe.read('a', transport, cursor=cursor, **options), graphql=transport == 'graphql')
            rejected(lambda: probe.read('ab', transport, cursor=cursor, filters=base, sort={'field': 'quantityLot', 'direction': 'desc'}), graphql=transport == 'graphql')
            rejected(lambda: probe.read('ab', transport, cursor=cursor, filters={**base, 'status': {'eq': 'Open'}}, sort=options['sort']), graphql=transport == 'graphql')
        grants_path = directory / 'grants.json'
        grants = json.loads(grants_path.read_text())
        try:
            revoked = json.loads(json.dumps(grants))
            revoked['epoch'] += 1
            revoked['principals']['b']['enabled'] = False
            write_private(grants_path, revoked)
            for transport in ('rest', 'graphql'):
                rejected(lambda: probe.read('b', transport, **options), status=401)
                visible = sorted([case for case in cases if 'a' in case.parties], key=lambda c: int(c.quantity))
                probe.pages('a', transport, visible, **options)
        finally:
            # Restoring access must not revive a cursor issued before the grant change.
            write_private(grants_path, {**grants, 'epoch': grants['epoch'] + 2})
        rejected(lambda: probe.read(cursor=cursor, **options))
        cursor = probe.read(**options)['nextCursor']
        # A positive resume now separates revision rejection from the earlier grant-epoch test.
        probe.read(cursor=cursor, **options)
        with probe.promoted():
            if probe.selected['revision'] != '2' or probe.selected['fingerprint'] == original['fingerprint']:
                raise ValueError('Outcomia revision promotion was not discovered')
            for transport in ('rest', 'graphql'):
                rejected(lambda: probe.read('ab', transport, cursor=cursor, **options), graphql=transport == 'graphql')
                for principal in ('a', 'b', 'ab'):
                    visible = [case for case in cases if principal == 'ab' or principal in case.parties]
                    for is_null in (True, False):
                        expected_cases = [case for case in visible if (case.reason is None) == is_null]
                        probe.pages(principal, transport, sorted(expected_cases, key=lambda c: c.contract), filters={**base, 'closeReason': {'isNull': is_null}})
                        sequences += 1
                    for direction in ('asc', 'desc'):
                        # Only A's contract has a reason; null values remain last in both directions.
                        ordered = sorted(visible, key=lambda c: (c.reason is None, c.contract))
                        probe.pages(principal, transport, ordered, filters=base, sort={'field': 'closeReason', 'direction': direction})
                        sequences += 1
        if probe.selected != original:
            raise ValueError('Outcomia original binding was not restored')
        generated = directory / 'generated'
        generated.mkdir(exist_ok=True)
        for action, name in [('typescript', 'models.ts'), ('graphql', 'models.graphql')]:
            (generated / name).write_text(probe.cli('models', action, directory / 'models-v2.json'))
        return {'result': 'passed', 'filtered_page_sequences': sequences, 'real_application_choices': True,
                'party_scopes': ['A-only', 'B-only', 'shared'], 'exact_values_and_derived_filters': True,
                'discovery_and_transport_parity': True, 'cursor_principal_filter_sort_revision_binding': True,
                'caller_revocation_and_epoch': True, 'optional_field_promotion_and_rollback': True, 'model_revisions': [1, 2, 1]}
    finally:
        worker.terminate()
        try:
            worker.wait(timeout=5)
        except subprocess.TimeoutExpired:
            worker.kill(); worker.wait()
