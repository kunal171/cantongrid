"""Opt-in M0 experiment against the disposable LocalNet, never a runtime connector."""
import hashlib
import json
from pathlib import Path
import secrets
import re
import subprocess
import time
from urllib.error import HTTPError, URLError
from zipfile import ZipFile

from ..common import ENV, ROOT, pg_command, write_private
from ..http import open_local
from ..ledger import get, request, token
from ..pqs import source_config
from ..visibility import ensure_user, right, rights_equal
from .exercise_evidence import compare_view

BASE = 'http://127.0.0.1:3975'
IMAGE = 'europe-docker.pkg.dev/da-images/public/docker/scribe@sha256:072faffbee3640448b282fc973b04ec99f5ba2bf92d2b43d01a00c89626c3a67'


def event_format(parties):
    """Select only explicit parties. A wildcard template is not wildcard party authority."""
    selection = {'cumulative': [{'identifierFilter': {'WildcardFilter': {'value': {
        'includeCreatedEventBlob': False}}}}]}
    return {'filtersByParty': {party: selection for party in parties}, 'verbose': True}


def exporter_config(directory):
    """One sequential exporter, bounded logs/memory, separate profile/database for every scope."""
    return {'services': {'probe': {
        'image': IMAGE, 'command': ['pipeline', 'ledger', 'postgres-document'],
        'user': f'{ENV["PQS_UID"]}:{ENV["PQS_GID"]}',
        'environment': {'SCRIBE_CONFIG': '/config/source.conf',
                        'JAVA_TOOL_OPTIONS': '-Xms128m -Xmx640m -XX:ActiveProcessorCount=2'},
        'volumes': [f'{directory}/source.conf:/config/source.conf:ro'],
        'ports': ['127.0.0.1:18083:8080'], 'mem_limit': '1g',
        'logging': {'driver': 'json-file', 'options': {'max-size': '10m', 'max-file': '3'}},
        'networks': ['source-db', 'localnet']}},
        'networks': {'source-db': {'external': True, 'name': 'cantongrid-pqs_default'},
                     'localnet': {'external': True, 'name': 'cantongrid-localnet'}}}


def sql(database, statement, reader=None):
    """Provision only owned experiment databases as owner; event SELECTs use a dedicated reader."""
    command = pg_command(database)
    if reader:
        # Replace the owner invocation completely; failures never fall back to it.
        command = pg_command(database, reader=True)
        command[command.index('cantongrid_pqs_reader')] = reader
    result = subprocess.run(command, input=statement, text=True, capture_output=True,
                            check=True, timeout=15, env=ENV)
    return result.stdout.strip()


def wait_exporter():
    """Wait at most one minute for all source readiness fields, without trusting cached SQL rows."""
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        try:
            with open_local('http://127.0.0.1:18083/readyz', timeout=2) as response:
                health = json.load(response)
            if health.get('status') == 'ok' and all(health.get(key) is True for key in
                                                   ('grpc_up', 'jdbc_connection_pool_up', 'stream_up')):
                return
        except (OSError, URLError, ValueError):
            pass
        time.sleep(0.5)
    raise ValueError('Isolated exercise exporter did not become ready')


def run(dar, resume=None):
    """Upload an owned fixture, submit controls and compare four currently authorized source views.

    Caller must enforce --allow-ledger-writes before entering this function.
    Raw results are private; only synthetic counts and comparison booleans print.
    """
    if resume is not None:
        if not re.fullmatch(r'cg-m0-[0-9a-f]{8}', resume):
            raise ValueError('Resume requires an owned exercise run ID')
        compare_sources(ROOT / '.local/exercise-source/runs' / resume)
        return
    payload = dar.read_bytes()
    with ZipFile(dar) as archive:
        # DAR manifests may fold long main-DALF filenames onto continuation lines.
        lines = []
        for line in archive.read('META-INF/MANIFEST.MF').decode().splitlines():
            if line.startswith(' '):
                lines[-1] += line[1:]
            else:
                lines.append(line)
        manifest = dict(line.split(': ', 1) for line in lines if ': ' in line)
    package = Path(manifest['Main-Dalf']).stem.rsplit('-', 1)[1]
    if len(package) != 64 or any(c not in '0123456789abcdef' for c in package):
        raise ValueError('Unsupported fixture package ID')
    identifier = 'cg-m0-' + secrets.token_hex(4)
    directory = ROOT / '.local/exercise-source/runs' / identifier
    admin = token()
    version = get(BASE, '/v2/version', admin)['version']
    request(BASE, '/v2/dars/validate', admin, body=payload,
            content_type='application/octet-stream', read_json=False, timeout=120)
    request(BASE, '/v2/dars?vetAllPackages=true', admin, body=payload,
            content_type='application/octet-stream', read_json=False, timeout=120)
    parties = {name: request(BASE, '/v2/parties', admin, body={
        'partyIdHint': identifier + '-' + name, 'identityProviderId': ''})['partyDetails']['party']
        for name in ('operator', 'a', 'b')}
    writer = identifier + '-writer'
    ensure_user(writer, [right('CanActAs', party) for party in parties.values()])
    scopes = {'operator': [parties['operator']], 'a': [parties['a']],
              'b': [parties['b']], 'ab': [parties['a'], parties['b']]}
    for scope, selected in scopes.items():
        ensure_user(identifier + '-' + scope, [right('CanReadAs', party) for party in selected])
    write_private(directory / 'context.json', {'parties': parties, 'scopes': scopes,
                  'package': package, 'dar_sha256': hashlib.sha256(payload).hexdigest(), 'version': version})
    start = int(get(BASE, '/v2/state/ledger-end', admin)['offset'])
    transactions = {}

    def template(entity):
        """Use the actual DAR package, not a package-name alias that could resolve another version."""
        return package + ':ExerciseProbe:' + entity

    def submit(name, command, actor='operator', expected_failure=None):
        """Record each completed control immediately; rejected internals are never treated as events."""
        try:
            response = request(BASE, '/v2/commands/submit-and-wait-for-transaction', token(writer), body={
                'commands': {'commandId': identifier + '-' + name, 'userId': writer,
                             'actAs': [parties[actor]], 'commands': [command]}}, timeout=120)
        except HTTPError as exc:
            body = exc.read().decode()
            write_private(directory / (name + '-rejection.json'), {'status': exc.code, 'body': body})
            if not expected_failure or exc.code != 400 or expected_failure not in body:
                raise ValueError('Fixture rejection did not match its intended control') from None
            return None
        if expected_failure:
            raise ValueError('Fixture negative control unexpectedly committed')
        transaction = response['transaction']
        transactions[name] = int(transaction['offset'])
        write_private(directory / (name + '-submission.json'), response)
        write_private(directory / 'transactions.json', transactions)
        return transaction

    factory = submit('factory', {'CreateCommand': {'templateId': template('Factory'), 'createArguments': {
        'operator': parties['operator'], 'audience': [parties['a'], parties['b']], 'factoryId': identifier}}})
    factory_id = next(event['CreatedEvent']['contractId'] for event in factory['events'] if 'CreatedEvent' in event)

    def exercise(name, choice, argument, **kwargs):
        """Exercise the captured current factory, never a guessed contract ID."""
        return submit(name, {'ExerciseCommand': {'templateId': template('Factory'),
                      'contractId': factory_id, 'choice': choice, 'choiceArgument': argument}}, **kwargs)

    exercise('pair', 'Factory_Pair', {'runId': identifier + '-pair',
             'leftReaders': [parties['a']], 'rightReaders': [parties['b']]})
    exercise('shared', 'Factory_Pair', {'runId': identifier + '-shared',
             'leftReaders': [parties['a'], parties['b']], 'rightReaders': []})
    exercise('unauthorized', 'Factory_Pair', {'runId': identifier + '-unauthorized',
             'leftReaders': [], 'rightReaders': []}, actor='a', expected_failure='DAML_AUTHORIZATION_ERROR')
    nested = exercise('nested', 'Factory_Nested', {'runId': identifier + '-nested', 'readers': [parties['a']]})
    factory_id = next(event['CreatedEvent']['contractId'] for event in nested['events']
                      if 'CreatedEvent' in event and event['CreatedEvent']['templateId'].endswith(':Factory'))
    exercise('rollback', 'Factory_Reject', {'runId': identifier + '-rejected'},
             expected_failure='Deliberate M0 rollback control')
    submit('forged', {'CreateCommand': {'templateId': template('Child'), 'createArguments': {
        'owner': parties['b'], 'readers': [], 'runId': identifier + '-forged',
        'label': 'direct', 'declaredFactory': identifier}}}, actor='b')
    end = int(get(BASE, '/v2/state/ledger-end', admin)['offset'])
    ledger = {}
    for scope, selected in scopes.items():
        ledger[scope] = {}
        for name, offset in transactions.items():
            try:
                response = request(BASE, '/v2/updates/transaction-by-offset', token(identifier + '-' + scope), body={
                    'offset': offset, 'transactionFormat': {'eventFormat': event_format(selected),
                    'transactionShape': 'TRANSACTION_SHAPE_LEDGER_EFFECTS'}})
            except HTTPError as exc:
                if exc.code != 404:
                    raise
                response = None
            ledger[scope][name] = response
        write_private(directory / ('ledger-' + scope + '.json'), ledger[scope])
    write_private(directory / 'boundaries.json', {'start_exclusive': start, 'end_inclusive': end})
    compare_sources(directory)


def compare_sources(directory):
    """Resume source checks from saved committed controls; never resubmit their Ledger transactions."""
    identifier = directory.name
    context = json.loads((directory / 'context.json').read_text())
    scopes, version = context['scopes'], context['version']
    parties = context['parties']
    expected_scopes = {'operator': [parties['operator']], 'a': [parties['a']],
                       'b': [parties['b']], 'ab': [parties['a'], parties['b']]}
    if scopes != expected_scopes or len(set(parties.values())) != 3:
        raise ValueError('Saved exercise source scope differs from the fixture contract')
    bounds = json.loads((directory / 'boundaries.json').read_text())
    start, end = bounds['start_exclusive'], bounds['end_inclusive']
    if type(start) is not int or type(end) is not int or not 0 <= start <= end <= 9223372036854775807:
        raise ValueError('Invalid exercise evidence bounds')
    transactions = json.loads((directory / 'transactions.json').read_text())
    ledger = {scope: json.loads((directory / ('ledger-' + scope + '.json')).read_text()) for scope in scopes}
    compose_path = directory / 'compose.json'
    write_private(compose_path, exporter_config(directory))
    compose = ['docker', 'compose', '--progress', 'plain', '-p', identifier, '-f', str(compose_path)]
    summary = {'evidence_kind': 'localnet_exercise_comparison', 'run_id': identifier, 'version': version,
               'schema_version': '041', 'direct_parent_supported': False,
               'lineage_enabled': False, 'scopes': {}}
    for scope, selected in scopes.items():
        verify_scope(identifier + '-' + scope, selected)
        database = identifier.replace('-', '_') + '_' + scope
        # Retrying exporter setup must not require new parties or duplicate transactions.
        sql('postgres', "SELECT 'CREATE DATABASE " + database + " OWNER pqs_owner' WHERE NOT EXISTS "
            "(SELECT 1 FROM pg_database WHERE datname='" + database + "')\\gexec")
        if sql('postgres', "SELECT datdba='pqs_owner'::regrole FROM pg_database WHERE datname='" + database + "';") != 't':
            raise ValueError('Experiment database has an unexpected owner')
        config = source_config(identifier + '-' + scope, ' | '.join(selected), '*', database)
        config['pipeline']['datasource'] = 'TransactionTreeStream'
        write_private(directory / 'source.conf', config)
        try:
            subprocess.run([*compose, 'up', '-d', '--force-recreate'], env=ENV, check=True,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=60)
            wait_exporter()
            role = database + '_reader'
            # These grants apply only to a new owned fixture database, never to an existing source profile.
            if sql('postgres', "SELECT EXISTS(SELECT 1 FROM pg_roles WHERE rolname='" + role + "');") != 't':
                sql(database, "CREATE ROLE " + role + " LOGIN PASSWORD 'local-pqs-reader';")
            safe = sql(database, "SELECT NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolreplication "
                "AND NOT rolbypassrls AND NOT EXISTS(SELECT 1 FROM pg_auth_members WHERE member=r.oid) "
                "AND NOT EXISTS(SELECT 1 FROM pg_database WHERE datdba=r.oid) "
                "AND NOT EXISTS(SELECT 1 FROM pg_class WHERE relowner=r.oid) "
                "FROM pg_roles r WHERE rolname='" + role + "';")
            if safe != 't':
                raise ValueError('Existing experiment reader has unexpected authority')
            sql(database, "REVOKE CREATE ON SCHEMA public FROM PUBLIC; REVOKE TEMPORARY ON DATABASE " + database + " FROM PUBLIC;\n"
                "GRANT CONNECT ON DATABASE " + database + " TO " + role + "; GRANT USAGE ON SCHEMA public TO " + role + ";\n"
                "GRANT SELECT ON ALL TABLES IN SCHEMA public TO " + role + "; ALTER ROLE " + role + " SET default_transaction_read_only=on;")
            restricted = sql(database, "SELECT NOT has_database_privilege('" + role + "',current_database(),'CREATE,TEMP') "
                "AND NOT has_schema_privilege('" + role + "','public','CREATE') "
                "AND NOT EXISTS(SELECT 1 FROM pg_class c WHERE relnamespace='public'::regnamespace "
                "AND relkind IN ('r','p') AND has_table_privilege('" + role + "',c.oid,'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER'));", role)
            if restricted != 't':
                raise ValueError('Experiment reader has unexpected write privileges')
            # A healthy stream may lag the fixed boundary; poll the retained offset before comparing.
            deadline = time.monotonic() + 30
            expected_boundary = max(transactions[name] for name, value in ledger[scope].items() if value is not None)
            while int(sql(database, 'SELECT latest_offset();', role)) < expected_boundary:
                if time.monotonic() > deadline:
                    raise ValueError('Exercise source did not reach the controlled transaction boundary')
                time.sleep(0.5)
            latest = int(sql(database, 'SELECT latest_offset();', role))
            # PQS rejects future offsets; an invisible final transaction need not advance
            # this source's checkpoint. Record that tail rather than inventing coverage.
            query_end = min(end, latest)
            statement = f"""BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout='5s';
SELECT json_build_object(
'exercises',(SELECT coalesce(json_agg(row_to_json(e)),'[]') FROM (SELECT * FROM exercises(NULL,{start},{query_end}) LIMIT 64) e),
'creates',(SELECT coalesce(json_agg(row_to_json(c)),'[]') FROM (SELECT * FROM creates(NULL,{start},{query_end}) LIMIT 64) c),
'archives',(SELECT coalesce(json_agg(row_to_json(c)),'[]') FROM (SELECT * FROM archives(NULL,{start},{query_end}) LIMIT 64) c));
ROLLBACK;"""
            raw = sql(database, statement, role)
            if len(raw.encode()) > 512 * 1024:
                raise ValueError('Fixture evidence exceeds its byte bound')
            evidence = json.loads(raw)
            if any(len(rows) >= 64 for rows in evidence.values()):
                raise ValueError('Fixture evidence reaches its row bound')
            verify_scope(identifier + '-' + scope, selected)
            profile = json.loads(sql(database, "SELECT json_build_object('schema_version',"
                "(SELECT version FROM flyway_schema_history WHERE success AND version ~ '^[0-9]+$' ORDER BY installed_rank DESC LIMIT 1),"
                "'postgres_version',current_setting('server_version'),'oldest_offset',oldest_offset(),"
                "'latest_offset',latest_offset(),'pruned_offset',pruned_offset());", role))
            if profile['schema_version'] != '041':
                raise ValueError('Unsupported live exercise source schema')
            write_private(directory / ('profile-' + scope + '.json'), profile)
            write_private(directory / ('pqs-' + scope + '.json'), evidence)
            summary['scopes'][scope] = compare_view(ledger[scope], evidence)
            summary['scopes'][scope]['paging'] = probe_paging(database, role, start, query_end, evidence['exercises'])
            summary['scopes'][scope]['transactions'] = probe_transaction_ids(database, role, start, query_end, ledger[scope])
            write_private(directory / ('coverage-' + scope + '.json'), {
                'requested_end': end, 'query_end': query_end, 'latest_offset': latest,
                'pre_snapshot_history_verified': False, 'pruning_verified': False})
            write_private(directory / 'summary.json', summary)
        finally:
            subprocess.run([*compose, 'down'], env=ENV, check=True, timeout=60,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    write_private(directory / 'summary.json', summary)
    print(json.dumps(summary, indent=2))


def verify_scope(user, selected):
    """Check exact, active read-only Ledger rights before/after source reads; never repair drift."""
    admin = token()
    actual_user = get(BASE, '/v2/users/' + user, admin)['user']
    rights = get(BASE, '/v2/users/' + user + '/rights', admin)['rights']
    if actual_user.get('isDeactivated') or not rights_equal(rights, [right('CanReadAs', p) for p in selected]):
        raise ValueError('Exercise source rights changed; no comparison is authorized')


def probe_paging(database, role, start, end, expected):
    """Research one-row keyset pages at a fixed boundary, including same-offset ties and overlap.

    No durable worker checkpoint changes. The ordinary runtime remains unchanged.
    At most 32 SELECT pages are allowed for this deliberately tiny fixture.
    """
    key = lambda row: (row['exercise_event_id']['offset'], row['exercise_event_id']['node_id'])
    rows = []
    last = (start, -1)
    for _ in range(32):
        query = f"""BEGIN READ ONLY; SET LOCAL statement_timeout='5s';
SELECT row_to_json(e) FROM exercises(NULL,{start},{end}) e
WHERE (exercised_at_offset,(exercise_event_id).node_id)>({last[0]},{last[1]})
ORDER BY exercised_at_offset,(exercise_event_id).node_id LIMIT 1; ROLLBACK;"""
        raw = sql(database, query, role)
        if not raw:
            break
        row = json.loads(raw)
        if not rows and raw != sql(database, query, role):
            raise ValueError('Overlapping exercise page changed')
        if key(row) <= last:
            raise ValueError('Exercise keyset failed to advance')
        rows.append(row)
        last = key(row)
    else:
        raise ValueError('Exercise paging reached its iteration bound')
    if rows != sorted(expected, key=key):
        raise ValueError('Keyset exercise pages differ from bounded source receipts')
    return {'page_size': 1, 'receipts': len(rows), 'overlap_exact': True, 'complete_at_observed_boundary': True}


def probe_transaction_ids(database, role, start, end, ledger):
    """Verify participant-local offset→update ID binding; never publish the private update IDs."""
    expected = {int(value['transaction']['offset']): value['transaction']['updateId']
                for value in ledger.values() if value is not None}
    query = f"""BEGIN READ ONLY; SET LOCAL statement_timeout='5s';
SELECT coalesce(json_agg(row_to_json(t)),'[]') FROM
(SELECT "offset",transaction_id FROM __transactions WHERE "offset" BETWEEN {start} AND {end}
ORDER BY "offset" LIMIT 32) t; ROLLBACK;"""
    rows = json.loads(sql(database, query, role))
    if len(rows) >= 32:
        raise ValueError('Transaction identity comparison reached its row bound')
    actual = {int(row['offset']): row['transaction_id'] for row in rows}
    if len(actual) != len(rows) or actual != expected:
        raise ValueError('PQS transaction identities differ from authorized Ledger Effects')
    return {'receipts': len(rows), 'update_ids_match': True}
