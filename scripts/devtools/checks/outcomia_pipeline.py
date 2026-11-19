"""Explicitly authorized, local-only Outcomia example using the unmodified Rust runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import time
from datetime import datetime, timedelta, timezone
from urllib.error import HTTPError, URLError
from urllib.request import Request
from ..http import open_local
from zipfile import ZipFile

from ..common import ROOT, ENV, compose, write_private, source_ready
from ..ledger import request, get, token
from ..visibility import BASE, WRITER, USERS, context
from .pipeline import application_token


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dar', type=Path, required=True)
    parser.add_argument('--allow-ledger-writes', action='store_true', required=True,
                        help='Required explicit opt-in; uploads DAR and submits an isolated local test lifecycle')
    args = parser.parse_args(argv)
    if not source_ready(True):
        raise ValueError('Visibility PQS is unavailable; renew its local credentials with pqs up --visibility before submitting test commands')
    binary = Path(os.environ.get('CANTONGRID_BIN', ROOT / 'target/debug/cantongrid')).resolve()
    payload = args.dar.read_bytes()
    with ZipFile(args.dar) as archive:
        lines = []
        for line in archive.read('META-INF/MANIFEST.MF').decode().splitlines():
            if line.startswith(' '):
                lines[-1] += line[1:]
            else:
                lines.append(line)
    manifest = dict(line.split(': ', 1) for line in lines if ': ' in line)
    package = Path(manifest['Main-Dalf']).stem.rsplit('-', 1)[1]
    request(BASE, '/v2/dars?vetAllPackages=true', token(), body=payload,
            content_type='application/octet-stream', timeout=120)
    parties = context()['parties']
    selected = list(parties.values())
    identifier = 'cantongrid-local-' + secrets.token_hex(6)
    directory = ROOT / '.local/outcomia-pipeline' / identifier
    protocol_template = '#outcomia-markets:Outcomia.Binary:Protocol'
    market_template = '#outcomia-markets:Outcomia.Binary:BinaryMarket'

    def active(template):
        offset = get(BASE, '/v2/state/ledger-end', token())['offset']
        selection = {'cumulative': [{'identifierFilter': {'TemplateFilter': {'value': {'templateId': template}}}}]}
        rows = request(BASE, '/v2/state/active-contracts?limit=100', token(WRITER), body={
            'activeAtOffset': offset, 'eventFormat': {'filtersByParty': {p: selection for p in selected}, 'verbose': True}})
        result = []
        for row in rows:
            entry = row.get('contractEntry', {}).get('JsActiveContract', {})
            created = entry.get('createdEvent')
            if created and created['createArgument'].get('protocolName') == identifier:
                result.append(created)
        return result

    expected_events = set()

    def submit(command, suffix):
        response = request(BASE, '/v2/commands/submit-and-wait-for-transaction', token(WRITER), body={
            'commands': {'commandId': identifier + suffix, 'userId': WRITER,
                         'actAs': selected, 'packageIdSelectionPreference': [package], 'commands': [command]}}, timeout=120)
        created = []
        for event in response['transaction']['events']:
            for tag, kind in [('CreatedEvent', 'create'), ('ArchivedEvent', 'archive')]:
                if tag in event:
                    expected_events.add((kind, event[tag]['contractId']))
                    if kind == 'create': created.append(event[tag])
        return created

    def exercise(template, contract, choice, arguments, suffix):
        return submit({'ExerciseCommand': {'templateId': template, 'contractId': contract,
                                   'choice': choice, 'choiceArgument': arguments}}, suffix)

    # Existing test parties and writer are reused; no balances, assets or trades are created.
    submit({'CreateCommand': {'templateId': protocol_template, 'createArguments': {
        'operator': parties['a'], 'protocolName': identifier, 'admins': selected,
        'nextMarketId': '1', 'takerFeeBps': '50', 'feeRevision': '0'}}}, '-protocol')
    protocol = active(protocol_template)[0]['contractId']
    key = secrets.token_hex(32)
    env = {**os.environ,
           'CANTONGRID_SOURCE_TOKEN': token(USERS['ab'], 3600),
           'CANTONGRID_OUTCOMIA_PQS_URL': 'host=127.0.0.1 port=5441 user=cantongrid_visibility_reader password=local-visibility-reader dbname=cantongrid_visibility',
           'CANTONGRID_DESTINATION_URL': 'host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid',
           'CANTONGRID_OUTCOMIA_API_KEY': key, 'CANTONGRID_OUTCOMIA_CURSOR_KEY': secrets.token_hex(32)}
    templates = [{'package_id': package, 'module': 'Outcomia.Binary', 'entity': name} for name in ('Protocol', 'BinaryMarket')]
    config = {'version': 1, 'authorization': {'endpoint': BASE, 'user': USERS['ab'], 'source': identifier,
               'network': 'localnet', 'generation': 1, 'parties': selected, 'credential_env': 'CANTONGRID_SOURCE_TOKEN'},
              'source_database_env': 'CANTONGRID_OUTCOMIA_PQS_URL', 'destination_database_env': 'CANTONGRID_DESTINATION_URL',
              'source_health': 'http://127.0.0.1:18081/readyz', 'templates': templates, 'page_size': 1,
              'history': 'snapshot_forward_partial', 'archive_policy': 'prior_create_current_grants',
              'payload': 'raw_json', 'retention': 'retain_while_authorized',
              'api': {'bind': '127.0.0.1:18889', 'issuer': 'cantongrid-local-check', 'audience': 'cantongrid-api-check',
                      'token_key_env': 'CANTONGRID_OUTCOMIA_API_KEY', 'cursor_key_env': 'CANTONGRID_OUTCOMIA_CURSOR_KEY',
                      'grants_file': 'grants.json'}}
    write_private(directory / 'runtime.json', config)
    write_private(directory / 'grants.json', {'version': 1, 'epoch': 1, 'principals': {
        name: {'enabled': True, 'parties': selected if name == 'ab' else [parties[name]]} for name in ('a', 'b', 'ab')}})
    args_index = [str(binary), 'index', str(directory / 'runtime.json'), '--once']
    # Wait for the existing PQS exporter to observe the protocol, then capture a real snapshot.
    source_query = [*compose(), 'exec', '-T', '-e', 'PGPASSWORD=local-visibility-reader',
                    'pqs-postgres', 'psql', '-X', '-qAt', '-h', '127.0.0.1',
                    '-U', 'cantongrid_visibility_reader', '-d', 'cantongrid_visibility', '-v', 'ON_ERROR_STOP=1']
    deadline = time.monotonic() + 20
    while True:
        literal = "'" + identifier.replace("'", "''") + "'"
        observed = subprocess.run(source_query, input="SELECT count(*) FROM active('Outcomia.Binary:Protocol') WHERE payload->>'protocolName'=" + literal + ';', env=ENV, text=True, capture_output=True, check=True)
        if observed.stdout.strip() == '1':
            break
        if time.monotonic() > deadline:
            raise ValueError('PQS did not observe the created protocol')
        time.sleep(0.2)
    subprocess.run(args_index, env=env, check=True, capture_output=True)
    # Terms reference a deliberately local test document, not a real market or funded asset.
    document = b'CantonGrid local prediction-market lifecycle test; no collateral or trading.\n'
    write_private(directory / 'rules.json', {'text': document.decode()})
    close = (datetime.now(timezone.utc) + timedelta(days=1)).isoformat().replace('+00:00', 'Z')
    terms = {'question': 'LocalNet test: will the fixture complete?', 'collateral': {'admin': parties['a'], 'id': 'LOCAL-TEST-ONLY'},
             'rules': {'yesCriteria': 'The local fixture completes.', 'noCriteria': 'The local fixture fails.',
                       'invalidCriteria': 'The fixture is interrupted.', 'evidenceRequirements': 'Local test report.',
                       'documentUri': 'https://example.invalid/cantongrid-localnet-rules',
                       'documentSha256': hashlib.sha256(document).hexdigest()},
             'closeAt': close, 'earliestResolutionAt': None, 'fallback': None,
             'resolutionDelaySeconds': '86400', 'priceTick': '0.0000000001', 'quantityLot': '9223372036854775807'}
    exercise(protocol_template, protocol, 'Protocol_CreateBinary', {'creator': parties['b'], 'terms': terms}, '-draft')
    protocol = active(protocol_template)[0]['contractId']
    market = active(market_template)[0]['contractId']
    exercise(market_template, market, 'Binary_Open', {'protocol': protocol}, '-open')
    market = active(market_template)[0]['contractId']
    exercise(market_template, market, 'Binary_Close', {'protocol': protocol, 'reason': 'Local integration test complete'}, '-close')
    time.sleep(1)
    # Single-event pages force same-offset tie handling; each invocation restarts the worker.
    worker_restarts = 1
    for _ in range(16):
        subprocess.run(args_index, env=env, check=True, capture_output=True)
        worker_restarts += 1
    process = subprocess.Popen([str(binary), 'serve', str(directory / 'runtime.json')], env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    base = 'http://127.0.0.1:18889'

    def query(path, subject, body=None):
        headers = {'Authorization': 'Bearer ' + application_token(subject, key)}
        if body is not None:
            headers['Content-Type'] = 'application/json'
        with open_local(Request(base + path, data=json.dumps(body).encode() if body is not None else None, headers=headers), timeout=15) as response:
            if response.headers.get('Cache-Control') != 'no-store':
                raise ValueError('Private Outcomia API response permits caching')
            return json.load(response)

    def pages(path):
        """Read all bounded pages; previous test runs may put this run beyond the first page."""
        items = []
        base_path = path + '?limit=25'
        for _ in range(64):
            page = query(path if '?' in path else base_path, 'ab')
            items.extend(page['items'])
            if not page['nextCursor']:
                return items
            path = base_path + '&cursor=' + page['nextCursor']
        raise ValueError('Outcomia acceptance exceeded its API page budget')

    try:
        deadline = time.monotonic() + 10
        while True:
            try:
                page = query('/v1/contracts', 'ab')
                break
            except (HTTPError, URLError):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise ValueError('Outcomia Rust API did not become ready') from None
                time.sleep(0.1)
        # Older local runs increase the snapshot size. Observe actual state/history instead
        # of assuming exactly 17 restarts are sufficient to reach this run's source offset.
        deadline = time.monotonic() + 90
        for _ in range(128):
            own = [item for item in pages('/v1/contracts') if json.loads(item['payload']['value'])['protocolName'] == identifier]
            history_items = pages('/v1/events')
            observed = {(item['kind'], item['contractId']) for item in history_items}
            if len(own) == 2 and expected_events.issubset(observed):
                market = next(json.loads(item['payload']['value']) for item in own if item['template']['entity'] == 'BinaryMarket')
                if market['status'] == 'Closed':
                    break
            if time.monotonic() > deadline:
                raise ValueError('Outcomia lifecycle did not catch up within its acceptance budget')
            subprocess.run(args_index, env=env, check=True, capture_output=True)
            worker_restarts += 1
            time.sleep(0.1)
        else:
            raise ValueError('Outcomia lifecycle exceeded its worker restart budget')
        if len(own) != 2:
            raise ValueError('Expected one current protocol and one current market')
        market = next(json.loads(item['payload']['value']) for item in own if item['template']['entity'] == 'BinaryMarket')
        if market['status'] != 'Closed' or market['terms']['quantityLot'] != '9223372036854775807' or market['terms']['priceTick'] != '0.0000000001':
            raise ValueError('Outcomia state or exact terms changed')
        # Verify the exact transaction-produced create/archive contract identities, not just
        # aggregate counts which could be inflated by unrelated test protocols.
        observed_events = set()
        path = '/v1/events?limit=25'
        while path:
            history = query(path, 'ab')
            for item in history['items']:
                if item['sourceOffset'] is None or 'envelope' in item or 'position' in item or 'witnesses' in item:
                    raise ValueError('Public provenance is incomplete or exposes internal source metadata')
            observed_events.update((item['kind'], item['contractId']) for item in history['items'])
            path = '/v1/events?limit=25&cursor=' + history['nextCursor'] if history['nextCursor'] else None
        if len(expected_events) != 8 or not expected_events.issubset(observed_events):
            raise ValueError('A controlled Outcomia lifecycle event was missed at the snapshot/update handoff')
        for principal in ('a', 'b', 'ab'):
            result = query('/graphql', principal, {'query': '{ activeContracts { visibleTotal provenance } lifecycleEvents { visibleTotal items } }'})
            if 'errors' in result or int(result['data']['lifecycleEvents']['visibleTotal']) < 8:
                raise ValueError('Outcomia GraphQL lifecycle is incomplete')
        from .outcomia_models import verify_models
        model_report = verify_models(binary=binary, directory=directory, env=env, query=query,
                                     package=package, parties=parties, identifier=identifier,
                                     terms=terms, protocol=protocol, submit=submit, exercise=exercise)
        # New market choices include intermediate archives. Reconcile every submitted identity,
        # not just the three final model rows that can survive a missed-event implementation.
        observed = {(item['kind'], item['contractId']) for item in pages('/v1/events')}
        if not expected_events.issubset(observed):
            raise ValueError('An Outcomia model-control lifecycle event was not indexed')
        creates = {contract for kind, contract in expected_events if kind == 'create'}
        archives = {contract for kind, contract in expected_events if kind == 'archive'}
        own = [item for item in pages('/v1/contracts') if json.loads(item['payload']['value'])['protocolName'] in {identifier, identifier + '-a', identifier + '-b'}]
        if {item['contractId'] for item in own} != creates - archives:
            raise ValueError('Outcomia canonical active set differs from submitted lifecycle events')
        write_private(directory / 'report.json', {
            'dar_sha256': hashlib.sha256(payload).hexdigest(), 'main_package_id': package,
            'outcomia_lifecycle': ['Protocol', 'Draft', 'Open', 'Closed'], 'own_active_records': len(own),
            'original_lifecycle_events': 8, 'controlled_lifecycle_events': len(expected_events), 'controlled_event_identity_set_matched': True, 'single_event_pages': True, 'worker_restarts': worker_restarts,
            'exact_price_tick_and_int64': True, 'rest_and_graphql': True,
            'ledger_writes': 'explicitly authorized local-only test', 'funded_assets_or_trades': False,
            'history': 'snapshot_forward_partial', 'model_queries': model_report})
        print('PASS: actual Outcomia lifecycle and model queries; exact values, private scopes, cursors, optional-field promotion/rollback and caller revocation')
    finally:
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
