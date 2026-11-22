"""Approved generic LocalNet controls through the real Rust worker; no fixture provisioning."""
import json
import os
from pathlib import Path
import re
import secrets
import subprocess
import time

from ..common import ROOT, ENV, write_private
from ..ledger import get, request, token
from ..pqs import source_config
from .exercise_localnet import BASE, event_format, exporter_config, verify_scope, wait_exporter, sql


def destination(statement):
    """Read only this experiment's separately owned destination through its restricted owner."""
    command = ['docker', 'compose', '-p', 'cantongrid-storage', '-f', str(ROOT / 'dev/storage.compose.yaml'),
        'exec', '-T', '-e', 'PGPASSWORD=local-cantongrid-only', 'postgres', 'psql', '-X', '-qAt',
        '-h', '127.0.0.1', '-U', 'cantongrid_owner', '-d', 'cantongrid', '-v', 'ON_ERROR_STOP=1']
    return subprocess.run(command, input=statement, text=True, capture_output=True,
                          check=True, timeout=15, env=ENV).stdout.strip()


def run(resume, *, lineage=False, scope='operator', outcomia=False):
    """Index a fresh snapshot, then the already approved pair/nested Daml controls.

    The dispatcher requires --allow-ledger-writes. Existing parties/users/DAR are
    reused without rights changes; source reads retain the exact operator scope.
    """
    if not resume or not re.fullmatch(r'cg-m0-[0-9a-f]{8}', resume):
        raise ValueError('Worker check requires the existing M0 --resume run ID')
    original = ROOT / '.local/exercise-source/runs' / resume
    context = json.loads((original / 'context.json').read_text())
    parties = context['parties']
    if outcomia:
        if not lineage:raise ValueError('Outcomia lineage check requires --lineage')
        from . import exercise_outcomia
        context={**context,'package':exercise_outcomia.PACKAGE}
    module='Outcomia.Binary' if outcomia else 'ExerciseProbe'
    entities=['Protocol','BinaryMarket'] if outcomia else ['Factory','Child','Relay']
    if scope not in ('operator','ab') or (scope!='operator' and not lineage):
        raise ValueError('Unsupported worker scope')
    selected = [parties['operator']] if scope=='operator' else [parties['a'],parties['b']]
    user = resume + '-' + scope
    verify_scope(user, selected)
    identifier = ('cg-m2-' if lineage else 'cg-m1-') + secrets.token_hex(4)
    directory = ROOT / '.local/exercise-source/workers' / identifier
    database = resume.replace('-', '_') + '_' + scope
    profile = source_config(user, ' | '.join(selected), '*', database)
    profile['pipeline']['datasource'] = 'TransactionTreeStream'
    write_private(directory / 'source.conf', profile)
    write_private(directory / 'compose.json', exporter_config(directory))
    compose = ['docker', 'compose', '--progress', 'plain', '-p', identifier, '-f', str(directory / 'compose.json')]
    config = {'version': 1, 'authorization': {'endpoint': BASE, 'user': user, 'source': identifier,
        'network': 'localnet', 'generation': 1, 'parties': selected, 'credential_env': 'CANTONGRID_M1_TOKEN'},
        'source_database_env': 'CANTONGRID_M1_PQS_URL', 'destination_database_env': 'CANTONGRID_DESTINATION_URL',
        'source_health': 'http://127.0.0.1:18083/readyz', 'exercise_source': 'pqs041_ledger_effects',
        'templates': [{'package_id': context['package'], 'module': module, 'entity': entity}
                      for entity in entities],
        'page_size': 1, 'history': 'snapshot_forward_partial', 'archive_policy': 'prior_create_current_grants',
        'payload': 'raw_json', 'retention': 'retain_while_authorized',
        'api': {'bind': '127.0.0.1:0', 'issuer': 'm1-test', 'audience': 'm1-test',
                'token_key_env': 'CANTONGRID_M1_API_KEY', 'cursor_key_env': 'CANTONGRID_M1_CURSOR_KEY', 'grants_file': 'unused.json'}}
    if lineage:
        config['lineage_source']='pqs041_complete_subtrees'
    write_private(directory / 'runtime.json', config)
    env = {**ENV, 'CANTONGRID_M1_TOKEN': token(user, 3600),
        'CANTONGRID_M1_PQS_URL': f'host=127.0.0.1 port=5441 user={database}_reader password=local-pqs-reader dbname={database}',
        'CANTONGRID_DESTINATION_URL': 'host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid'}
    binary = os.environ.get('CANTONGRID_BIN', str(ROOT / 'target/debug/cantongrid'))

    def cli(*args):
        """Capture diagnostics privately; print no tokens, arguments or contract identities."""
        result = subprocess.run([binary, *args], env=env, capture_output=True, check=False, timeout=30)
        if result.returncode:
            write_private(directory / 'worker-error.json', {'output': result.stderr.decode(errors='replace')})
            raise ValueError('Rust exercise worker failed; private diagnostic saved')

    def checkpoint():
        """Read only the uniquely named worker stream, never another application's state."""
        raw = destination("SELECT checkpoint FROM cantongrid.streams WHERE definition->>'source'='" + identifier + "';")
        return json.loads(raw) if raw else None

    try:
        subprocess.run([*compose, 'up', '-d'], env=ENV, check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=60)
        wait_exporter()
        cli('storage', 'init', '--database-env', 'CANTONGRID_DESTINATION_URL')
        if outcomia:
            factories=[exercise_outcomia.bootstrap(resume+'-writer',parties,identifier)]
        else:
            selection = {'activeAtOffset': get(BASE, '/v2/state/ledger-end', token())['offset'],
                         'eventFormat': event_format(selected)}
            rows = request(BASE, '/v2/state/active-contracts?limit=100', token(resume + '-writer'), body=selection)
            factories = [row['contractEntry']['JsActiveContract']['createdEvent'] for row in rows
                         if 'JsActiveContract' in row.get('contractEntry', {})]
            factories = [row for row in factories if row['templateId'].endswith(':ExerciseProbe:Factory')
                         and row['createArgument']['factoryId'] == resume]
        if len(factories) != 1:
            raise ValueError('Expected one existing owned factory; no contract was guessed')
        # An already healthy exporter can still be replaying older approved controls.
        # Reach the current factory's actual creation offset before choosing the snapshot.
        deadline=time.monotonic()+45
        while int(sql(database,'SELECT latest_offset();',database+'_reader'))<int(factories[0]['offset']):
            if time.monotonic()>deadline:raise ValueError('Source did not catch up before the snapshot')
            time.sleep(0.2)
        for _ in range(64):
            cli('index',str(directory/'runtime.json'),'--once')
            if checkpoint()['snapshot'] is False:break
        else:raise ValueError('Worker snapshot exceeded its iteration bound')
        controls=exercise_outcomia.actions(parties,identifier) if outcomia else [('Factory_Pair', {'runId': identifier + '-pair',
                'leftReaders': [parties['a']], 'rightReaders': [parties['b']]}),
                ('Factory_Nested', {'runId': identifier + '-nested', 'readers': [parties['a']]})]
        for choice, arguments in controls:
            response = request(BASE, '/v2/commands/submit-and-wait-for-transaction', token(resume + '-writer'), body={
                'commands': {'commandId': identifier + '-' + choice, 'userId': resume + '-writer', 'actAs': [parties['operator']],
                    'commands': [{'ExerciseCommand': {'templateId': factories[0]['templateId'],
                        'contractId': factories[0]['contractId'], 'choice': choice, 'choiceArgument': arguments}}]}}, timeout=120)
            write_private(directory / (choice + '.json'), response)
            offset = int(response['transaction']['offset'])
            deadline = time.monotonic() + 45
            while True:
                cli('index', str(directory / 'runtime.json'), '--once')
                if checkpoint()['boundary'] >= offset:
                    break
                if time.monotonic() > deadline:
                    raise ValueError('Worker did not acknowledge the controlled transaction')
                time.sleep(0.2)
        summary_query = """
SELECT json_build_object(
    'exercises', (SELECT count(*) FROM cantongrid.exercise_events e WHERE e.stream_key=s.stream_key),
    'exercise_outbox', (SELECT count(*) FROM cantongrid.exercise_outbox e WHERE e.stream_key=s.stream_key),
    'consuming', (SELECT count(*) FROM cantongrid.exercise_events e
        WHERE e.stream_key=s.stream_key AND (e.envelope->>'consuming')::boolean),
    'unindexed_targets', (SELECT count(*) FROM cantongrid.exercise_events e
        WHERE e.stream_key=s.stream_key AND NOT EXISTS (
            SELECT 1 FROM cantongrid.contracts c
            WHERE c.stream_key=e.stream_key AND c.contract_id=e.envelope->>'contract')),
    'lifecycle_archives', (SELECT count(*) FROM cantongrid.events e
        WHERE e.stream_key=s.stream_key AND e.kind='archive'))
FROM cantongrid.streams s WHERE definition->>'source'='""" + identifier + "';"
        report = json.loads(destination(summary_query))
        count=1 if outcomia else (4 if scope=='operator' else 3)
        expected={'exercises':count,'exercise_outbox':count,'consuming':1 if outcomia else 3,'unindexed_targets':0 if outcomia else 2,'lifecycle_archives':1}
        if report != expected:
            raise ValueError('Worker receipt/outbox counts differ from the four expected exercises')
        cli('index', str(directory / 'runtime.json'), '--once')
        if json.loads(destination(summary_query)) != report:
            raise ValueError('Idle worker cycle duplicated exercise evidence')
        verify_scope(user, selected)
        if lineage:
            from .exercise_graph import verify_graph
            report['graph']=verify_graph(directory,config,env,binary,parties,scope,factories[0]['contractId'],outcomia=outcomia)
        write_private(directory / 'summary.json', report)
        print(json.dumps(report))
    finally:
        subprocess.run([*compose, 'down'], env=ENV, check=True, timeout=60,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
