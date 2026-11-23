"""Read-only Ledger acceptance for grouped counts/sums; destination revisions are restored."""
from collections import Counter, defaultdict
from urllib.error import HTTPError
import json

import support as s
import queries


def ledger_totals():
    """Expected values come from a real authorized Ledger snapshot, independently of projections."""
    offset = s.get(s.BASE, '/v2/state/ledger-end', s.token())['offset']
    counts = Counter(); totals = defaultdict(int)
    for contract in s.ledger_snapshot(offset):
        payload = contract['payload']
        counts[payload['state']] += 1
        # Python integers are arbitrary precision: adding two Int64 maxima never rounds.
        if not isinstance(payload['quantityLot'], (int, str)) or isinstance(payload['quantityLot'], bool):
            raise ValueError('Ledger quantity must be an exact integer')
        totals[payload['state']] += int(payload['quantityLot'])
    if not counts:
        raise ValueError('Run the standard PoC before checking aggregates')
    return {state: {'state': state, 'contracts': str(count), 'lotSum': str(totals[state])}
            for state, count in counts.items()}


def run():
    """Temporarily bind revision 4, compare both transports, then restore the original binding."""
    # Finish any previously committed source prefix without submitting ledger commands.
    for _ in range(32):
        before = s.metrics()
        s.cli('index', s.DIRECTORY / 'runtime.json', '--once')
        if s.metrics() == before:
            break
    else:
        raise ValueError('Source did not reach a stable bounded fixture head')
    expected = ledger_totals()
    if len(expected) < 2:
        raise ValueError('Complete the normal PoC to provide multiple states for pagination controls')
    declaration = {'group_by': ['state'], 'measures': {
        'contracts': {'op': 'count'}, 'lotSum': {'op': 'sum', 'field': 'quantityLot'}}}
    data = s.state(); first_cursor = None
    with s.process('serve'):
        s.wait(lambda: bool(s.query()), 'PoC API did not become ready')
        with queries.promoted(revision=4, aggregation=declaration):
            described = s.api('ab', queries.endpoint())
            field = described['aggregateQueryField']
            seen = {}; cursor = None
            for _ in range(16):
                rest = s.api('ab', queries.endpoint() + '/aggregate', {'limit': 1, 'cursor': cursor})
                text = f'''query($namespace:String!,$cursor:String){{
                  {field}(namespace:$namespace,limit:1,cursor:$cursor){{
                    visibleTotal nextCursor fingerprint
                    items{{groupKey values{{state contracts lotSum}}}}
                  }}
                }}'''
                gql = s.api('ab', '/models/graphql', {'query': text,
                    'variables': {'namespace': data['namespace'], 'cursor': cursor}})
                if gql.get('errors'):
                    raise ValueError('Generated aggregate GraphQL failed')
                graph = gql['data'][field]
                if rest['items'] != graph['items'] or rest['visibleTotal'] != graph['visibleTotal']:
                    raise ValueError('Aggregate transports disagree')
                if rest['visibleTotal'] != str(len(expected)):
                    raise ValueError('Visible aggregate count differs from Ledger snapshot')
                for row in rest['items']:
                    values = row['values']
                    if values != expected.get(values['state']) or values['state'] in seen:
                        raise ValueError('Aggregate values differ from exact Ledger inputs')
                    seen[values['state']] = values
                cursor = graph['nextCursor']  # Cross-transport reuse of an authenticated cursor.
                first_cursor = first_cursor or cursor
                if not cursor:
                    break
            else:
                raise ValueError('Aggregate exceeded its pagination budget')
            if seen != expected:
                raise ValueError('Aggregate pagination missed a Ledger group')
            for principal in ('a', 'b'):
                try:
                    s.api(principal, queries.endpoint() + '/aggregate', {})
                except HTTPError as error:
                    if error.code != 403:
                        raise
                else:
                    raise ValueError('Partial source scope obtained an aggregate')
            if first_cursor:
                queries.rejected(lambda: s.api('ab', queries.endpoint() + '/query', {'cursor': first_cursor}))
        # Successful queries through the old generated root prove the active revision was restored.
        if not s.query():
            raise ValueError('Original model query was not restored')
    s.write_private(s.DIRECTORY / 'aggregate-report.json', {
        'result': 'passed', 'source': 'authorized Ledger snapshot and LocalNet PQS',
        'inputContracts': sum(int(v['contracts']) for v in expected.values()),
        'groups': len(expected), 'exactCountAndSum': True, 'restGraphqlParity': True,
        'crossTransportCursors': True, 'rowCursorIsolation': True, 'narrowScopesDenied': True,
        'originalRevisionRestored': True, 'ledgerWrites': False})
    print('PASS: real Ledger totals match generated aggregate REST/GraphQL, cursors and scope guards')
