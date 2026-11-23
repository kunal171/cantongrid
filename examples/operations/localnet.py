"""Read-only-ledger acceptance over an existing PoC; writes only an isolated owned destination stream."""
import argparse
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import shlex
import socket
import subprocess
import sys
import time
from urllib.error import HTTPError
from urllib.request import Request
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'examples/poc'))
import support  # noqa: E402
from devtools.common import write_private  # noqa: E402
from devtools.http import open_local  # noqa: E402


def sql_literal(value):
    """Escape PostgreSQL data literals only; no supplied value becomes SQL syntax or a shell command."""
    return "'" + str(value).replace("'", "''") + "'"


def query(connection, sql):
    """Keep the disposable fixture password out of argv; SQL travels through stdin."""
    fields = dict(part.split('=', 1) for part in shlex.split(connection))
    if fields.get('host') != '127.0.0.1' or set(fields) - {'host', 'port', 'user', 'password', 'dbname'}:
        raise ValueError('This acceptance helper supports only the explicit numeric-loopback fixture')
    environment = {**os.environ, **{f'PG{key}': fields[name] for key, name in
                   [('HOST', 'host'), ('PORT', 'port'), ('USER', 'user'),
                    ('PASSWORD', 'password'), ('DATABASE', 'dbname')] if name in fields}}
    result = subprocess.run(['psql', '-X', '-qAt', '-v', 'ON_ERROR_STOP=1'],
                            env=environment, input=sql, text=True, capture_output=True)
    if result.returncode:
        raise ValueError('Owned acceptance SQL failed; no connection values are printed')
    return result.stdout.strip()


def bearer(key, issuer, audience, subject='reader'):
    """Mint only a short-lived local organization fixture bearer, never a Canton command credential."""
    encode = lambda data: base64.urlsafe_b64encode(json.dumps(data).encode()).rstrip(b'=').decode()
    now = int(time.time())
    head = encode({'alg': 'HS256', 'typ': 'JWT'})
    body = encode({'sub': subject, 'iss': issuer, 'aud': audience, 'iat': now, 'exp': now + 900})
    signature = base64.urlsafe_b64encode(hmac.new(key.encode(), f'{head}.{body}'.encode(), hashlib.sha256).digest()).rstrip(b'=').decode()
    return f'{head}.{body}.{signature}'


def request(base, token, path, body=None):
    """Use the existing loopback/no-proxy/no-redirect client and cap each response before decoding."""
    headers = {'Authorization': f'Bearer {token}'}
    data = None
    if body is not None:
        headers['Content-Type'] = 'application/json'
        data = json.dumps(body).encode()
    try:
        with open_local(Request(base + path, data=data, headers=headers), timeout=15) as response:
            status, payload = response.status, response.read(2 * 1024 * 1024 + 1)
    except HTTPError as error:
        status, payload = error.code, error.read(2 * 1024 * 1024 + 1)
    if len(payload) > 2 * 1024 * 1024:
        raise ValueError('Acceptance response exceeds the runtime budget')
    return status, json.loads(payload)


def run(*, reconciliation=False):
    """Check retained LocalNet data through isolated destination operations or exact reconciliation."""
    if reconciliation and subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip():
        raise ValueError('Commit reviewed changes before recording LocalNet reconciliation evidence')
    original_files = [support.DIRECTORY / name for name in ('runtime.json', 'models.json', 'grants.json', 'state.json')]
    original_digests = [hashlib.sha256(path.read_bytes()).hexdigest() for path in original_files]
    config = json.loads((ROOT / '.local/poc/runtime.json').read_text())
    if config['authorization']['network'] != 'localnet' or config['authorization'].get('transport', 'localnet_http') != 'localnet_http':
        raise ValueError('Only the existing disposable LocalNet PoC is supported')
    identity = 'ops-acceptance-' + uuid.uuid4().hex
    directory = ROOT / '.local/operations' / identity
    directory.mkdir(parents=True, mode=0o700)
    config['authorization']['source'] = identity
    config['authorization']['generation'] = 1
    with socket.socket() as listener:
        listener.bind(('127.0.0.1', 0))
        port = listener.getsockname()[1]
    config['api'].update(bind=f'127.0.0.1:{port}', issuer='operations-acceptance',
                         audience='operations-api', grants_file='grants.json')
    write_private(directory / 'grants.json', {'version': 1, 'epoch': 1, 'principals': {
        'reader': {'enabled': True, 'parties': config['authorization']['parties']}}})
    path = directory / 'runtime.json'
    write_private(path, config)
    environment = support.environment()
    binary = str(support.BINARY)
    log = directory / 'commands.log'
    log.touch(mode=0o600)

    def cli(*arguments):
        """Product operations execute in Rust; failures do not echo environment credentials."""
        with log.open('a') as errors:
            result = subprocess.run([binary, *map(str, arguments)], env=environment,
                                    text=True, stdout=subprocess.PIPE, stderr=errors)
        if result.returncode:
            raise ValueError('Rust acceptance operation failed; inspect the private commands.log')
        return json.loads(result.stdout) if result.stdout.strip() else None

    cli('ops', 'preflight', path, '--profile', 'localnet')
    destination = environment[config['destination_database_env']]
    original_namespace = support.state()['namespace']
    original_bindings_sql = "SELECT COALESCE(jsonb_agg(to_jsonb(b) ORDER BY stream_key,name),'[]') FROM cantongrid.model_bindings b WHERE namespace=" + sql_literal(original_namespace)
    original_bindings = query(destination, original_bindings_sql)
    selected = 'definition->>\'source\'=' + sql_literal(identity)
    for pages in range(1, 1025):
        cli('index', path, '--once')
        checkpoint = json.loads(query(destination, f'SELECT checkpoint FROM cantongrid.streams WHERE {selected}'))
        if not checkpoint['snapshot']:
            break
    else:
        raise ValueError('Snapshot exceeded the bounded acceptance budget')
    boundary = checkpoint['boundary']
    parties = 'ARRAY[' + ','.join(map(sql_literal, config['authorization']['parties'])) + ']::text[]'
    selection = ' OR '.join('(c.creation_package_id=' + sql_literal(t['package_id']) +
                            " AND split_part(c.template_fqn,':',2)=" + sql_literal(t['module']) +
                            " AND split_part(c.template_fqn,':',3)=" + sql_literal(t['entity']) + ')'
                            for t in config['templates'])
    source_select = (
        "SELECT c.contract_id,c.payload FROM public.active(NULL," + str(int(boundary)) + ") c "
        "WHERE NOT c.divulged_only AND c.payload_type::text='template' AND "
        f'c.witnesses && {parties} AND ({selection})')
    if reconciliation:
        source_select += ' ORDER BY c.contract_id LIMIT 129'
    source_rows = json.loads(query(environment[config['source_database_env']],
        "SELECT COALESCE(jsonb_agg(jsonb_build_object('contractId',c.contract_id,'payload',c.payload) "
        "ORDER BY c.contract_id),'[]') FROM (" + source_select + ') c'))
    minimum = 1 if reconciliation else 5
    if len(source_rows) < minimum:
        raise ValueError('Acceptance lacks the required existing PoC inventory; no ledger writes were performed')
    reconciliation_context = None
    if reconciliation:
        import reconciliation as checks
        reconciliation_context = checks.configure(config, directory, identity, cli, source_rows, boundary)
    token = bearer(environment[config['api']['token_key_env']], config['api']['issuer'], config['api']['audience'])
    base = f'http://127.0.0.1:{port}'
    child = None

    def start():
        """Restart only the API; source Ledger/PQS and durable owned state remain unchanged."""
        process = subprocess.Popen([binary, 'serve', str(path)], env=environment,
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        for _ in range(100):
            try:
                if request(base, token, '/v1/health')[0] == 200:
                    return process
            except OSError:
                pass
            time.sleep(0.05)
        process.kill()
        process.wait()
        raise ValueError('Acceptance API did not become ready')

    try:
        child = start()
        if reconciliation:
            report, expected = checks.verify(config, directory, source_rows, environment, base, token,
                                             reconciliation_context, request, bearer)
            child.kill(); child.wait(); child = start()
            status, repeated = request(base, token, '/v1/reconciliation/poc_terms/query', {})
            if status != 200 or repeated != expected:
                raise ValueError('Reconciliation changed after API restart at the same indexed boundary')
            if original_digests != [hashlib.sha256(path.read_bytes()).hexdigest() for path in original_files] or query(destination, original_bindings_sql) != original_bindings:
                raise ValueError('Original PoC files or model bindings changed during isolated acceptance')
            report.update(original_poc_files_unchanged=True, original_poc_bindings_unchanged=True,
                          binary_sha256=hashlib.sha256(support.BINARY.read_bytes()).hexdigest())
            report.update(api_restart_equal=True, tested_commit=subprocess.check_output(
                ['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip())
            write_private(directory / 'evidence.json', report)
            print(json.dumps({'evidence_directory': str(directory), 'result': report}, indent=2))
            return
        # A fixed opaque snapshot handle replaces the unsafe snapshot-then-start-now sequence.
        status, bootstrap = request(base, token, '/v1/subscriptions/bootstrap', {'name': 'atomic-baseline'})
        assert status == 200 and bootstrap['atomicHandoff'] is True
        assert not {'asOfOffset', 'checkpoint', 'baseline_offset', 'baseline_after'} & bootstrap.keys()
        bootstrap_id = bootstrap['id']
        private_boundary = query(destination, 'SELECT baseline_offset FROM cantongrid.delivery_subscriptions WHERE id=' + sql_literal(bootstrap_id))
        assert int(private_boundary) == int(boundary)
        baseline_rows, baseline_cursor, baseline_pages = [], None, 0
        while True:
            baseline_path = bootstrap['snapshotUrl'] + '?limit=3' + (f'&cursor={baseline_cursor}' if baseline_cursor else '')
            status, page = request(base, token, baseline_path)
            assert status == 200 and page['provenance']['coverage'] == 'observed_retained'
            baseline_rows.extend({'contractId': row['contractId'], 'payload': json.loads(row['payload']['value'])} for row in page['items'])
            baseline_pages += 1
            baseline_cursor = page['nextCursor']
            if baseline_pages == 1:
                # Persisted baseline metadata and page cursors survive a process restart.
                child.kill()
                child.wait()
                child = start()
            if baseline_cursor is None:
                break
        assert baseline_rows == source_rows and baseline_pages >= 2
        assert request(base, token, f'/v1/subscriptions/{bootstrap_id}/events')[1]['events'] == []
        assert request(base, token, '/v1/subscriptions/bootstrap', {'name': 'atomic-baseline'})[1]['id'] == bootstrap_id
        assert request(base, token, f'/v1/subscriptions/{bootstrap_id}/cancel', {})[0] == 200
        rows, cursor, historical_pages = [], None, 0
        while True:
            page_path = f'/v1/contracts/as-of?asOfOffset={boundary}&limit=3' + (f'&cursor={cursor}' if cursor else '')
            status, page = request(base, token, page_path)
            assert status == 200 and page['provenance']['coverage'] == 'observed_retained'
            historical_pages += 1
            rows.extend({'contractId': row['contractId'], 'payload': json.loads(row['payload']['value'])} for row in page['items'])
            cursor = page['nextCursor']
            if cursor is None:
                break
        assert rows == source_rows
        status, sub = request(base, token, '/v1/subscriptions', {'name': 'replay', 'start': 'retained'})
        assert status == 200
        endpoint = f"/v1/subscriptions/{sub['id']}"
        status, first = request(base, token, endpoint + '/events?limit=3')
        assert status == 200 and len(first['events']) == 3
        assert all(event['snapshotObservation'] for event in first['events'])
        assert all(event['payload']['type'] == 'raw_json' for event in first['events'])
        child.kill()
        child.wait()
        child = start()
        assert request(base, token, endpoint + '/events?limit=3')[1]['events'] == first['events']
        assert request(base, token, endpoint + '/acknowledge', {'cursor': first['nextCursor']})[0] == 200
        next_page = request(base, token, endpoint + '/events?limit=3')[1]
        assert not {e['eventId'] for e in first['events']} & {e['eventId'] for e in next_page['events']}
        removed = 0
        while True:
            result = cli('ops', 'prune-deliveries', path, '--retain', '1', '--limit', '256')
            removed += int(result['removed'])
            if int(result['removed']) == 0:
                break
        status, gap = request(base, token, endpoint + '/events')
        assert status == 409 and gap['coverage'] == 'gap'
        status, historical = request(base, token, f'/v1/contracts/as-of?asOfOffset={boundary}')
        assert status == 200 and int(historical['visibleTotal']) == len(source_rows)
        assert request(base, token, endpoint + '/cancel', {})[0] == 200
        status, recovered = request(base, token, '/v1/subscriptions', {'name': 'after-baseline', 'start': 'now'})
        assert status == 200
        assert request(base, token, f"/v1/subscriptions/{recovered['id']}/acknowledge", {'cursor': recovered['cursor']})[0] == 200
        assert request(base, token, f"/v1/subscriptions/{recovered['id']}/events")[1]['events'] == []
        report = {'format_version': 1, 'scope': 'existing_localnet_read_only_ledger',
                  'tested_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
                  'contracts_compared': len(source_rows), 'snapshot_pages': pages, 'historical_pages': historical_pages,
                  'source_inventory_equal': True, 'historical_paging': True, 'api_restart_replay': True,
                  'durable_ack': True, 'explicit_retention_gap': True, 'outbox_rows_pruned': removed,
                  'canonical_history_preserved': True, 'quiescent_fixture_baseline_resync': True,
                  'atomic_snapshot_stream_handoff': False,
                  'atomic_bootstrap_inventory_equal': True, 'atomic_bootstrap_api_restart': True,
                  'atomic_bootstrap_feed_anchored': True, 'bootstrap_snapshot_pages': baseline_pages,
                  'atomic_bootstrap_concurrent_ledger_writes_tested': False, 'ledger_writes': False,
                  'ledger_grants_changed': False, 'remote_testnet_verified': False}
        write_private(directory / 'recovery.json', {'config': str(path), 'subscription': recovered['id'],
                      'boundary': boundary, 'source': identity})
        write_private(directory / 'evidence.json', report)
        print(json.dumps({'evidence_directory': str(directory), 'result': report}, indent=2))
    finally:
        if child is not None:
            child.kill()
            child.wait()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--allow-owned-writes', action='store_true',
                        help='Allow isolated destination ingestion, subscription progress and outbox retention')
    parser.add_argument('--reconciliation', action='store_true',
                        help='Check exact PoC policies through REST/GraphQL, without subscriptions or source writes')
    args = parser.parse_args()
    if not args.allow_owned_writes:
        parser.error('this acceptance modifies an isolated owned destination stream; use --allow-owned-writes')
    run(reconciliation=args.reconciliation)
