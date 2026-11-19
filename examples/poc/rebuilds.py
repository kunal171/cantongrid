"""Write-enabled LocalNet acceptance for private, bounded model rebuilds."""
import json
import secrets

import support as s
import queries
from devtools.visibility import right

# 261 exceeds the 128-row atomic binding cap and requires multiple 32-row steps.
ROWS = 261


def command(action, job, *args):
    """Every operation starts a fresh CLI process and renews real source authorization."""
    return json.loads(s.cli('models', 'rebuild', action, s.DIRECTORY / 'runtime.json', job, *args))['result']


def denied(call):
    """Require a rejected CLI operation; an unexpected success must fail acceptance."""
    try:
        call()
    except s.CliError:
        return
    raise ValueError('Expected rebuild rejection did not occur')


def finish(job, phase):
    """Bound the complete run as well as each individual database step."""
    for _ in range(100):
        status = command('step', job, '--limit', 32)
        if status['phase'] == phase:
            return status
    raise ValueError('LocalNet rebuild exceeded its bounded step budget')


def submit(commands, marker):
    """Submit only owned test commands; persist actual event identities before proceeding."""
    data = s.state()
    result = s.request(s.BASE, '/v2/commands/submit-and-wait-for-transaction', s.token(s.WRITER), body={
        'commands': {'commandId': marker, 'userId': s.WRITER,
                     'actAs': list(data['parties'].values()),
                     'packageIdSelectionPreference': [data['package']], 'commands': commands}}, timeout=120)
    created = []
    for event in result['transaction']['events']:
        if 'CreatedEvent' in event:
            created.append(event['CreatedEvent']['contractId'])
    return created


def create_rows(marker, start, count):
    """Alternate A-private, B-private and shared records; preserve exact values beyond 2**53."""
    parties = s.state()['parties']
    commands = []
    for index in range(start, start + count):
        role = ('a', 'b', 'shared')[index % 3]
        commands.append({'CreateCommand': {'templateId': s.TEMPLATE, 'createArguments': {
            'operator': parties['b' if role == 'b' else 'a'],
            'readers': [parties['b']] if role == 'shared' else [],
            'demoId': marker + '-' + str(index), 'question': marker,
            'priceTick': '0.0000000001', 'quantityLot': '9007199254740993',
            'state': 'Draft', 'outcome': None}}})
    created = submit(commands, marker + '-create-' + str(start))
    if len(created) != count:
        raise ValueError('Batch response did not contain every submitted contract')
    return {cid: ('a', 'b', 'shared')[(start + offset) % 3] for offset, cid in enumerate(created)}


def rows(marker, principal='ab'):
    """Use generated GraphQL with a run-specific filter, excluding unrelated source contracts."""
    return s.pages(principal, filters={'question': {'eq': marker}})


def verify_rows(marker, expected):
    """Compare contract identities and precise values for each independent caller scope."""
    for principal in ('a', 'b', 'ab'):
        visible = {cid for cid, role in expected.items() if principal == 'ab' or role in (principal, 'shared')}
        actual = rows(marker, principal)
        if {row['contractId'] for row in actual} != visible:
            raise ValueError('Staged model has missing or cross-party contract identities')
        if any(row['values']['quantityLot'] != '9007199254740993'
               or row['values']['priceTickBps'] != '0.0000010000' for row in actual):
            raise ValueError('Staged model changed an exact financial string')


def run(resume=None):
    """Keep revision 1 serving while real creates/archives arrive between rebuild pages."""
    data = s.state()
    if queries.metadata()['revision'] != '1':
        raise ValueError('Run the standard PoC first and restore revision 1')
    if resume is not None:
        # Resume only initial fixture setup, before any rebuild or lifecycle mutations.
        journal = resume.resolve()
        if journal.parent != s.DIRECTORY.resolve():
            raise ValueError('Resume requires a private journal in the PoC directory')
        saved = json.loads(journal.read_text())
        marker, expected = saved['marker'], saved['contracts']
        if journal.stem != marker or len(expected) != ROWS:
            raise ValueError('Only the initial 261-contract journal can resume setup')
        verify_rows(marker, expected)
    else:
        marker = 'rebuild-' + secrets.token_hex(6)
        journal = s.DIRECTORY / (marker + '.json')
        expected = {}
    original_runtime = json.loads((s.DIRECTORY / 'runtime.json').read_text())
    runtime = {**original_runtime, 'page_size': 8}
    original_field = data['field']
    catalog = json.loads((s.DIRECTORY / 'models.json').read_text())
    catalog['models'][0]['revision'] = 3
    model_file = s.DIRECTORY / 'rebuild-models.json'
    s.write_private(model_file, catalog)
    s.write_private(s.DIRECTORY / 'runtime.json', runtime)
    s.cli('models', 'register', model_file, '--namespace', data['namespace'], '--database-env', 'CANTONGRID_DESTINATION_URL')
    s.cli('models', 'install', data['namespace'], 'markets', 3, '--database-env', 'CANTONGRID_DESTINATION_URL')
    promoted = False
    try:
        # Explicit batches keep requests bounded and make ambiguous submissions inspectable.
        if resume is None:
            for start in range(0, ROWS, 32):
                expected.update(create_rows(marker, start, min(32, ROWS - start)))
                s.write_private(journal, {'marker': marker, 'contracts': expected})
        with s.process('index'):
            s.wait(lambda: len(rows(marker)) == ROWS, 'Large source fixture was not indexed', timeout=180)
        denied(lambda: s.cli('models', 'bind', s.DIRECTORY / 'runtime.json', data['namespace'], 'markets', 3, '--replace', 1))
        command('start', marker, data['namespace'], 'markets', 3, '--replace', 1)
        for _ in range(16):
            if command('step', marker, '--limit', 32)['scanned'] != '0':
                break
        else:
            raise ValueError('Rebuild did not begin its bounded scan')
        before = command('status', marker)
        # No long-lived process or in-memory cursor is retained between these two calls.
        if command('status', marker) != before:
            raise ValueError('Restart changed persisted rebuild progress')
        verify_rows(marker, expected)
        if queries.metadata()['revision'] != '1':
            raise ValueError('An inactive staged revision became visible')
        # Revoke the actual source reader's B grant, not an HTTP-only mock authority.
        path = '/v2/users/' + s.USERS['ab'] + '/rights'
        change = {'userId': s.USERS['ab'], 'identityProviderId': '',
                  'rights': [right('CanReadAs', data['parties']['b'])]}
        try:
            s.request(s.BASE, path, s.token(), method='PATCH', body=change)
            denied(lambda: command('step', marker, '--limit', 32))
        finally:
            s.request(s.BASE, path, s.token(), body=change)
        # Exporter readiness may lag grant restoration; wait without widening the scope.
        from devtools.common import wait_source
        wait_source(True)
        if command('status', marker) != before:
            raise ValueError('Denied source access advanced rebuild progress')
        # Archive one already indexed input, then add inputs while the worker was stopped.
        archived = next(iter(expected))
        submit([{'ExerciseCommand': {'templateId': s.TEMPLATE, 'contractId': archived,
                                    'choice': 'Archive', 'choiceArgument': {}}}], marker + '-archive')
        del expected[archived]
        expected.update(create_rows(marker, ROWS, 3))
        s.write_private(journal, {'marker': marker, 'contracts': expected})
        with s.process('index'):
            s.wait(lambda: {r['contractId'] for r in rows(marker)} == set(expected), 'Restart missed concurrent source changes')
        ready = finish(marker, 'ready')
        # A further real create after readiness must block activation until reconciliation.
        expected.update(create_rows(marker, ROWS + 3, 1))
        s.write_private(journal, {'marker': marker, 'contracts': expected})
        with s.process('index'):
            s.wait(lambda: len(rows(marker)) == len(expected), 'Post-ready create was not indexed')
        denied(lambda: command('activate', marker))
        finish(marker, 'ready')
        # A lost activation acknowledgement is ambiguous; require inspection too.
        promoted = True
        command('activate', marker)
        metadata = queries.metadata()
        current = s.state(); current['field'] = metadata['queryField']; s.save(current)
        verify_rows(marker, expected)
        finish(marker, 'complete')
        rollback = marker + '-rollback'
        command('start', rollback, data['namespace'], 'markets', 1, '--replace', 3)
        finish(rollback, 'ready'); command('activate', rollback)
        promoted = False
        current = s.state(); current['field'] = original_field; s.save(current)
        finish(rollback, 'complete')
        verify_rows(marker, expected)
        # Retire only this runner's own contracts; retain the ledger and indexer history.
        identifiers = list(expected)
        for offset in range(0, len(identifiers), 32):
            submit([{'ExerciseCommand': {'templateId': s.TEMPLATE, 'contractId': cid,
                                        'choice': 'Archive', 'choiceArgument': {}}}
                    for cid in identifiers[offset:offset + 32]], marker + '-cleanup-' + str(offset))
        with s.process('index'):
            s.wait(lambda: not rows(marker), 'Test contract cleanup was not indexed', timeout=180)
        s.write_private(s.DIRECTORY / 'rebuild-report.json', {
            'result': 'passed', 'initialRows': ROWS, 'finalRows': len(expected),
            'boundedStep': 32, 'freshProcessResume': True, 'sourceRevocation': True,
            'concurrentCreateArchive': True, 'postReadyActivationRejected': True,
            'privateStaging': True, 'partyIsolation': True, 'exactValues': True,
            'stagedPromotionRollback': True, 'source': 'real LocalNet PQS',
            'readyScanned': ready['scanned']})
    finally:
        # Never overwrite an active job or hide failed rollback. Retain its private journal
        # and require operator inspection when a promotion was not safely restored.
        if promoted:
            raise ValueError('Inspect staged revision 3 and its journal before resuming')
        s.write_private(s.DIRECTORY / 'runtime.json', original_runtime)
    print('PASS: 261-row LocalNet rebuild, restart, source revocation, concurrent events and staged rollback')
