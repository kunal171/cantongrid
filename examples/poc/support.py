"""Local demonstration plumbing. Product ingestion and APIs always run in the Rust CLI."""
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from contextlib import contextmanager
from urllib.error import HTTPError, URLError
from urllib.request import Request
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from devtools.http import open_local  # noqa: E402
from devtools.common import write_private, compose, ENV  # noqa: E402
from devtools.ledger import request, get, token  # noqa: E402
from devtools.visibility import BASE, WRITER, USERS, context  # noqa: E402
from devtools.checks.pipeline import application_token  # noqa: E402

# Runtime state and generated artifacts are owned by this demo and ignored by git.
DIRECTORY = ROOT / '.local/poc'
DAR = ROOT / 'examples/poc/.daml/dist/cantongrid-poc-0.1.0.dar'
API = 'http://127.0.0.1:18890'
BINARY = Path(os.environ.get('CANTONGRID_BIN', ROOT / 'target/debug/cantongrid')).resolve()
TEMPLATE = '#cantongrid-poc:Market:PredictionMarket'


def state():
    """Secrets and opaque local ledger identities stay in an ignored owner-only directory."""
    return json.loads((DIRECTORY / 'state.json').read_text())


def save(data):
    """Atomically persist owned fixture state, including keys and real ledger identities."""
    write_private(DIRECTORY / 'state.json', data)


def package_id():
    """Read the built DAR's actual main package; never guess a deployed package ID."""
    with ZipFile(DAR) as archive:
        lines = []
        for line in archive.read('META-INF/MANIFEST.MF').decode().splitlines():
            # Manifest continuation lines are folded before reading the Main-Dalf entry.
            if line.startswith(' '):
                lines[-1] += line[1:]
            else:
                lines.append(line)
    manifest = dict(line.split(': ', 1) for line in lines if ': ' in line)
    return Path(manifest['Main-Dalf']).stem.rsplit('-', 1)[1]


def environment():
    """Give Rust children the read-only source scope and separate organization/cursor keys."""
    data = state()
    return {**os.environ,
            'CANTONGRID_SOURCE_TOKEN': token(USERS['ab'], 3600),
            'CANTONGRID_POC_PQS_URL': 'host=127.0.0.1 port=5441 user=cantongrid_visibility_reader password=local-visibility-reader dbname=cantongrid_visibility',
            'CANTONGRID_DESTINATION_URL': 'host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid',
            'CANTONGRID_POC_API_KEY': data['apiKey'], 'CANTONGRID_POC_CURSOR_KEY': data['cursorKey']}


class CliError(ValueError):
    """An actual child CLI rejection, distinct from malformed evidence or failed assertions."""


def cli(*args):
    """Keep credentials in the child environment, never command arguments or public output."""
    result = subprocess.run([str(BINARY), *map(str, args)], env=environment(), capture_output=True, text=True)
    if result.returncode:
        raise CliError('CantonGrid command failed: ' + ' '.join(str(a) for a in args[:2]))
    return result.stdout


def api(subject, path, body=None):
    """Send fixture-authenticated requests without printing bearers or private response bodies."""
    data = state()
    payload = None if body is None else json.dumps(body).encode()
    with open_local(Request(API + path, data=payload, headers={
            'Authorization': 'Bearer ' + application_token(subject, data['apiKey']),
            'Content-Type': 'application/json'}), timeout=20) as response:
        result = json.load(response)
        if response.headers.get('Cache-Control') != 'no-store':
            raise ValueError('Private API response must not be cacheable')
    return result


def query(subject='ab', *, typed=True, limit=25, cursor=None,
          filters=None, sort=None, transport='graphql'):
    """Exercise independent API authentication; exact numbers stay strings in either transport."""
    data = state()
    if typed and transport == 'rest':
        body = {'limit': limit, 'cursor': cursor, 'filters': filters or {}}
        if sort is not None:
            body['sort'] = sort
        return api(subject, f"/v1/models/{data['namespace']}/markets/query", body)
    if transport != 'graphql':
        raise ValueError('History supports GraphQL only in this local helper')
    if typed:
        field = data['field']
        # The validated declaration fingerprint names both generated inputs and the served schema.
        stem = 'Model_' + field.rsplit('_', 1)[1]
        text = f'''query(
          $namespace: String!, $limit: Int!, $cursor: String,
          $filters: {stem}Filter, $sort: {stem}Sort
        ) {{
          {field}(namespace: $namespace, limit: $limit, cursor: $cursor,
                  filters: $filters, sort: $sort) {{
            visibleTotal nextCursor fingerprint
            items {{ contractId values {{ demoId question state outcome priceTick priceTickBps quantityLot }} }}
          }}
        }}'''
        path = '/models/graphql'
        body = {'query': text, 'variables': {'namespace': data['namespace'], 'limit': limit,
                'cursor': cursor, 'filters': filters, 'sort': sort}}
    else:
        if filters is not None or sort is not None:
            raise ValueError('Model filters cannot be applied to generic history')
        path = '/graphql'
        body = {'query': 'query($cursor:String){lifecycleEvents(limit:25,cursor:$cursor){visibleTotal nextCursor items provenance}}', 'variables': {'cursor': cursor}}
    result = api(subject, path, body)
    if result.get('errors'):
        raise ValueError('GraphQL operation failed')
    return result['data'][data['field'] if typed else 'lifecycleEvents']


def pages(subject='ab', *, typed=True, **options):
    """Follow at most 100 pages while retaining the exact same query semantics."""
    result = []; cursor = None
    # The same filters/sort travel with every cursor; changing them starts a new query.
    for _ in range(100):
        page = query(subject, typed=typed, cursor=cursor, **options)
        result.extend(page['items'])
        cursor = page['nextCursor']
        if not cursor:
            return result
    raise ValueError('Demo exceeded its bounded API pagination budget')


def wait(check, description, timeout=60):
    """Poll observable state, not fixed sleeps that can hide exporter/worker races."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            if check():
                return
        except (HTTPError, URLError, ConnectionError):
            pass
        time.sleep(0.3)
    raise ValueError(description)


@contextmanager
def process(command):
    """Terminate only children started by this invocation; never trust a saved PID file."""
    child = subprocess.Popen([str(BINARY), command, str(DIRECTORY / 'runtime.json')],
                             env=environment(), stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        yield child
    finally:
        if child.poll() is None:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill(); child.wait()


def metrics():
    """Read only test-owned destination counts, to verify restart/idempotence without raw data dumps."""
    data = state()
    # Counts are scoped to the saved source and exclude changing freshness timestamps,
    # so an unchanged result after restart proves no durable data/cursor movement.
    sql = """SELECT json_build_object('batches',(SELECT count(*) FROM cantongrid.batches b WHERE b.stream_key=s.stream_key),
        'events',(SELECT count(*) FROM cantongrid.events e WHERE e.stream_key=s.stream_key),
        'contracts',(SELECT count(*) FROM cantongrid.contracts c WHERE c.stream_key=s.stream_key),
        'checkpoint',checkpoint) FROM cantongrid.streams s WHERE definition->>'source' = :'source';"""
    cmd = ['docker', 'compose', '-p', 'cantongrid-storage', '-f', str(ROOT / 'dev/storage.compose.yaml'), 'exec', '-T', 'postgres', 'psql', '-XqAt', '-U', 'cantongrid_owner', '-d', 'cantongrid', '-v', 'ON_ERROR_STOP=1', '-v', 'source=' + data['source']]
    result = subprocess.run(cmd, input=sql, env=ENV, text=True, capture_output=True, check=True)
    return json.loads(result.stdout)


def ledger_snapshot(offset):
    """Read existing PoC contracts at an exact Ledger offset; reject truncation or unexpected templates."""
    data = state()
    expected = data['package'] + ':Market:PredictionMarket'
    selection = {'cumulative': [{'identifierFilter': {'TemplateFilter': {'value': {'templateId': TEMPLATE}}}}]}
    entries = request(BASE, '/v2/state/active-contracts?limit=100', token(USERS['ab']), body={
        'activeAtOffset': offset, 'eventFormat': {
            'filtersByParty': {party: selection for party in data['parties'].values()}, 'verbose': True}})
    if not isinstance(entries, list) or len(entries) >= 100:
        raise ValueError('Ledger snapshot exceeds the bounded PoC acceptance inventory')
    contracts = []
    for entry in entries:
        contract = entry.get('contractEntry', {})
        if 'JsActiveContract' not in contract:
            if contract and 'JsEmpty' not in contract:
                raise ValueError('Unsupported incomplete/reassigned contract in PoC snapshot')
            continue
        created = contract['JsActiveContract']['createdEvent']
        if created['templateId'] != expected:
            raise ValueError('Ledger snapshot returned an unexpected package/template')
        contracts.append({'contractId': created['contractId'], 'payload': created['createArgument']})
    if len(contracts) != len({contract['contractId'] for contract in contracts}):
        raise ValueError('Ledger snapshot contains duplicate contract identities')
    return sorted(contracts, key=lambda contract: contract['contractId'])
