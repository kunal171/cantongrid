"""Explicit, local-only DAR deployment and market choices for the reproducible PoC."""
import hashlib
import json
import secrets
from support import (ROOT, DIRECTORY, DAR, BASE, WRITER, USERS, TEMPLATE, package_id,
                     context, get, request, token, write_private, state, save, cli)


def configure():
    """Reuse verified fixture parties/users, but use a separate source identity and model namespace."""
    # Saved state ties source/model identity to the DAR and party scope. It also holds
    # private API keys, the latest contract IDs and the expected create/archive history.
    data = state() if (DIRECTORY / 'state.json').exists() else {
        'source': 'poc-' + secrets.token_hex(6), 'namespace': 'poc_' + secrets.token_hex(6),
        'apiKey': secrets.token_hex(32), 'cursorKey': secrets.token_hex(32),
        'expectedEvents': [], 'stage': 'unseeded', 'cycle': 0}
    package = package_id()
    digest = hashlib.sha256(DAR.read_bytes()).hexdigest()
    parties = context()['parties']
    # Reusing an old checkpoint with a different package or scope would mix two sources.
    if data.get('darSha256', digest) != digest or data.get('parties', parties) != parties:
        raise ValueError('Saved demo uses a different DAR or party scope; use a reviewed new demo directory')
    data.update(package=package, darSha256=digest, parties=parties)
    save(data)
    selected = list(parties.values())
    # One-event pages force the acceptance run to exercise durable cursor handoff.
    config = {'version': 1, 'authorization': {'endpoint': BASE, 'user': USERS['ab'],
        'source': data['source'], 'network': 'localnet', 'generation': 1,
        'parties': selected, 'credential_env': 'CANTONGRID_SOURCE_TOKEN'},
        'source_database_env': 'CANTONGRID_POC_PQS_URL', 'destination_database_env': 'CANTONGRID_DESTINATION_URL',
        'source_health': 'http://127.0.0.1:18081/readyz',
        'templates': [{'package_id': package, 'module': 'Market', 'entity': 'PredictionMarket'}],
        'page_size': 1, 'history': 'snapshot_forward_partial',
        'archive_policy': 'prior_create_current_grants', 'payload': 'raw_json', 'retention': 'retain_while_authorized',
        'api': {'bind': '127.0.0.1:18890', 'issuer': 'cantongrid-local-check', 'audience': 'cantongrid-api-check',
                'token_key_env': 'CANTONGRID_POC_API_KEY', 'cursor_key_env': 'CANTONGRID_POC_CURSOR_KEY', 'grants_file': 'grants.json'}}
    write_private(DIRECTORY / 'runtime.json', config)
    grants_path = DIRECTORY / 'grants.json'
    # Reconfiguration must not revive cursors issued before an earlier acceptance run.
    epoch = json.loads(grants_path.read_text())['epoch'] + 1 if grants_path.exists() else 1
    write_private(grants_path, {'version': 1, 'epoch': epoch, 'principals': {
        name: {'enabled': True, 'parties': selected if name == 'ab' else [parties[name]]} for name in ('a', 'b', 'ab')}})
    models = json.loads((ROOT / 'examples/poc/models.json').read_text())
    models['models'][0]['template']['package_id'] = package
    write_private(DIRECTORY / 'models.json', models)
    # Generated artifacts contain model output types, not tokens, parties or live contract data.
    generated = DIRECTORY / 'generated'; generated.mkdir(exist_ok=True)
    for command, filename in [('typescript', 'models.ts'), ('graphql', 'schema.graphql'), ('plan', 'schema-plan.json')]:
        (generated / filename).write_text(cli('models', command, DIRECTORY / 'models.json'))
    plan = json.loads((generated / 'schema-plan.json').read_text())['plans'][0]
    # This suffix matches the compiler's generated root; it changes with model revision.
    data['field'] = 'model_markets_' + plan['fingerprint'][:16]
    save(data)


def deploy():
    """The writer submits test commands; indexing continues with a different read-only identity."""
    data = state()
    if data['package'] not in get(BASE, '/v2/packages', token())['packageIds']:
        for path in ('/v2/dars/validate', '/v2/dars?vetAllPackages=true'):
            request(BASE, path, token(), body=DAR.read_bytes(), content_type='application/octet-stream', read_json=False, timeout=120)
    if data['stage'] == 'unseeded':
        for role in ('a', 'b', 'shared'):
            if role not in state().get('contracts', {}):
                create(role)


def submit(command, marker, expected_creates=1):
    """Persist the actual ledger event identities for verification against indexed API history."""
    data = state()
    # Source + marker keeps command identity scoped to this saved run/cycle. A lost
    # response needs ledger inspection before retrying; local state is not ledger truth.
    response = request(BASE, '/v2/commands/submit-and-wait-for-transaction', token(WRITER), body={
        'commands': {'commandId': data['source'] + '-' + marker, 'userId': WRITER,
                     'actAs': list(data['parties'].values()), 'packageIdSelectionPreference': [data['package']],
                     'commands': [command]}}, timeout=120)
    created = []
    for event in response['transaction']['events']:
        for tag, kind in [('CreatedEvent', 'create'), ('ArchivedEvent', 'archive')]:
            if tag in event:
                cid = event[tag]['contractId']
                # An immutable revision has one create and later one archive with the
                # same contract ID, so the event kind is part of the expected identity.
                data['expectedEvents'].append([kind, cid])
                if kind == 'create':
                    created.append(cid)
    save(data)
    if len(created) != expected_creates:
        raise ValueError('Expected exactly one created market revision')
    return created[0] if created else None


def create(role='shared'):
    """Create one fixture market; retire the resolved shared revision before a new cycle."""
    data = state()
    if role == 'shared':
        if data['stage'] == 'Resolved':
            submit({'ExerciseCommand': {'templateId': TEMPLATE, 'contractId': data['contracts']['shared'],
                'choice': 'Archive', 'choiceArgument': {}}}, 'archive-' + str(data['cycle']), expected_creates=0)
            data = state(); data['stage'] = 'Archived'; save(data)
        if data['stage'] not in ('unseeded', 'Archived'):
            raise ValueError('Finish the current shared market before starting another cycle')
        data['cycle'] += 1
        save(data)
    suffix = f"{role}-{data['cycle']}"
    owner = data['parties']['b' if role == 'b' else 'a']
    cid = submit({'CreateCommand': {'templateId': TEMPLATE, 'createArguments': {
        'operator': owner, 'readers': [data['parties']['b']] if role == 'shared' else [],
        'demoId': data['source'] + '-' + suffix, 'question': 'Will this local demonstration complete?',
        'priceTick': '0.0000000001', 'quantityLot': '9223372036854775807', 'state': 'Draft', 'outcome': None}}}, 'create-' + suffix)
    data = state()
    data.setdefault('contracts', {})[role] = cid
    if role == 'shared':
        data['stage'] = 'Draft'
    save(data)


def exercise(choice):
    """Consume the current shared revision and save the successor returned by the choice."""
    data = state()
    expected = 'Draft' if choice == 'Publish' else 'Open'
    if data['stage'] != expected:
        raise ValueError('Choice does not match the saved market stage')
    cid = submit({'ExerciseCommand': {'templateId': TEMPLATE, 'contractId': data['contracts']['shared'],
        'choice': choice, 'choiceArgument': {} if choice == 'Publish' else {'result': True}}}, choice.lower() + '-' + str(data['cycle']))
    # Consuming choices return a new contract ID; keep polls and later choices on it.
    data = state(); data['contracts']['shared'] = cid
    data['stage'] = 'Open' if choice == 'Publish' else 'Resolved'
    save(data)
