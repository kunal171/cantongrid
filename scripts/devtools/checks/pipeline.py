"""Exercise the Rust LocalNet pipeline against existing Visibility records; no ledger writes."""
import base64
import hashlib
import hmac
import json
import os
from pathlib import Path
import secrets
import subprocess
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request
from ..http import open_local

from ..common import ENV, ROOT, compose, write_private, wait_source
from ..ledger import token
from ..visibility import AMOUNT, SEQUENCE, USERS, context


def application_token(subject, key):
    """Issue an ephemeral organization test token, separate from LocalNet source auth."""
    def encode(value):
        """Encode compact JWT segments for the fixture organization bearer."""
        return base64.urlsafe_b64encode(json.dumps(value).encode()).rstrip(b'=')
    now = int(time.time())
    message = b'.'.join([encode({'alg': 'HS256', 'typ': 'JWT'}), encode({
        'sub': subject, 'iss': 'cantongrid-local-check', 'aud': 'cantongrid-api-check',
        'iat': now, 'exp': now + 600})])
    signature = base64.urlsafe_b64encode(hmac.new(key.encode(), message, hashlib.sha256).digest()).rstrip(b'=')
    return (message + b'.' + signature).decode()


def main():
    """Use the isolated AB SQL reader and existing source identity, never admin data reads."""
    binary = Path(os.environ.get('CANTONGRID_BIN', ROOT / 'target/debug/cantongrid'))
    if not binary.is_file():
        raise ValueError('Build the Rust CLI and set CANTONGRID_BIN to its executable')
    parties = context()['parties']
    command = [*compose(), 'exec', '-T', '-e', 'PGPASSWORD=local-visibility-reader',
               'pqs-postgres', 'psql', '-X', '-qAt', '-h', '127.0.0.1',
               '-U', 'cantongrid_visibility_reader', '-d', 'cantongrid_visibility', '-v', 'ON_ERROR_STOP=1']
    result = subprocess.run(command, input="SELECT creation_package_id FROM active('Visibility:Record') WHERE payload_type::text='template' LIMIT 1", text=True, capture_output=True, check=True, env=ENV)
    package = result.stdout.strip()
    if len(package) != 64 or any(c not in '0123456789abcdef' for c in package):
        raise ValueError('Existing visibility template was not found; no fixture was created')
    directory = ROOT / '.local/pipeline'
    config = {
        'version': 1,
        'authorization': {'endpoint': 'http://127.0.0.1:3975', 'user': USERS['ab'],
                          'source': 'localnet-visibility-bound', 'network': 'localnet', 'generation': 1,
                          'parties': list(parties.values()), 'credential_env': 'CANTONGRID_PIPELINE_SOURCE_TOKEN'},
        'source_database_env': 'CANTONGRID_PIPELINE_SOURCE_URL',
        'destination_database_env': 'CANTONGRID_PIPELINE_DESTINATION_URL',
        'source_health': 'http://127.0.0.1:18081/readyz',
        'templates': [{'package_id': package, 'module': 'Visibility', 'entity': 'Record'}],
        'history': 'snapshot_forward_partial', 'archive_policy': 'prior_create_current_grants',
        'payload': 'raw_json', 'retention': 'retain_while_authorized',
        'api': {'bind': '127.0.0.1:18889', 'issuer': 'cantongrid-local-check',
                'audience': 'cantongrid-api-check', 'token_key_env': 'CANTONGRID_PIPELINE_API_KEY',
                'cursor_key_env': 'CANTONGRID_PIPELINE_CURSOR_KEY', 'grants_file': 'grants.json'},
    }
    grants = {'version': 1, 'epoch': 1, 'principals': {
        'a': {'enabled': True, 'parties': [parties['a']]},
        'b': {'enabled': True, 'parties': [parties['b']]},
        'ab': {'enabled': True, 'parties': list(parties.values())}}}
    write_private(directory / 'runtime.json', config)
    write_private(directory / 'grants.json', grants)
    key = secrets.token_hex(32)
    env = {**os.environ,
           'CANTONGRID_PIPELINE_SOURCE_TOKEN': token(USERS['ab'], 3600),
           'CANTONGRID_PIPELINE_SOURCE_URL': 'host=127.0.0.1 port=5441 user=cantongrid_visibility_reader password=local-visibility-reader dbname=cantongrid_visibility',
           'CANTONGRID_PIPELINE_DESTINATION_URL': 'host=127.0.0.1 port=5442 user=cantongrid_owner password=local-cantongrid-only dbname=cantongrid',
           'CANTONGRID_PIPELINE_API_KEY': key, 'CANTONGRID_PIPELINE_CURSOR_KEY': secrets.token_hex(32)}
    for _ in range(3):
        subprocess.run([str(binary), 'index', str(directory / 'runtime.json'), '--once'], env=env, check=True)
    # Each invocation reopened the destination and source; the cursor survives those restarts.
    process = subprocess.Popen([str(binary), 'serve', str(directory / 'runtime.json')], env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    base = 'http://127.0.0.1:18889'

    def query(path, subject='ab', body=None):
        """Call the child Rust API with an independently signed fixture caller identity."""
        headers = {'Authorization': 'Bearer ' + application_token(subject, key)}
        if body is not None:
            headers['Content-Type'] = 'application/json'
        request = Request(base + path, data=json.dumps(body).encode() if body else None, headers=headers)
        with open_local(request, timeout=15) as response:
            if response.headers.get('Cache-Control') != 'no-store':
                raise ValueError('Private API response permits caching')
            return json.load(response)

    try:
        deadline = time.monotonic() + 10
        while True:
            try:
                query('/v1/contracts')
                break
            except (URLError, HTTPError):
                if process.poll() is not None or time.monotonic() > deadline:
                    raise ValueError('Rust API did not become ready') from None
                time.sleep(0.1)
        for subject, count in [('a', 2), ('b', 2), ('ab', 3)]:
            page = query('/v1/contracts', subject)
            if page['visibleTotal'] != str(count) or len(page['items']) != count:
                raise ValueError('Live source/API visibility count differs')
            labels = set()
            for item in page['items']:
                if item['payload']['type'] != 'raw_json':
                    raise ValueError('Unexpected payload mode')
                payload = json.loads(item['payload']['value'])
                labels.add(payload['label'])
                if payload['amount'] != AMOUNT or payload['sequence'] != SEQUENCE:
                    raise ValueError('Exact source numeric strings changed')
            expected_labels = {'a': {'a-only', 'joint'}, 'b': {'b-only', 'joint'}, 'ab': {'a-only', 'b-only', 'joint'}}
            if labels != expected_labels[subject]:
                raise ValueError('Live API returned a record outside the caller scope')
            graphql = query('/graphql', subject, {'query': '{ activeContracts { visibleTotal provenance } lifecycleEvents { visibleTotal } }'})
            if ('errors' in graphql or graphql['data']['activeContracts']['visibleTotal'] != str(count)
                    or graphql['data']['lifecycleEvents']['visibleTotal'] != str(count)):
                raise ValueError('Live GraphQL scoped count differs')
        first = query('/v1/contracts?limit=1', 'a')
        cursor = first['nextCursor']
        second = query('/v1/contracts?limit=1&cursor=' + cursor, 'a')
        if first['items'][0]['contractId'] == second['items'][0]['contractId']:
            raise ValueError('Live pagination repeated a record')
        # Keep SQL/cache data intact while the exporter is down; neither the worker nor
        # either API transport may mistake retained rows for a currently healthy source.
        stopped = False
        try:
            subprocess.run([*compose(True), 'stop', 'pqs-ab'], env=ENV, check=True, stdout=subprocess.DEVNULL)
            stopped = True
            failed = subprocess.run([str(binary), 'index', str(directory / 'runtime.json'), '--once'], env=env,
                                    capture_output=True, text=True)
            if failed.returncode == 0:
                raise ValueError('Worker accepted an unavailable source')
            for path, body in [('/v1/contracts', None), ('/graphql', {'query': '{ activeContracts { items } }'})]:
                try:
                    query(path, 'ab', body)
                except HTTPError as error:
                    if error.code != 503:
                        raise ValueError('Unexpected source-outage API status') from None
                else:
                    raise ValueError('API served private data while source was unavailable')
        finally:
            if stopped:
                subprocess.run([*compose(True), 'start', 'pqs-ab'], env=ENV, check=True, stdout=subprocess.DEVNULL)
                wait_source(True)
        if query('/v1/contracts', 'ab')['visibleTotal'] != '3':
            raise ValueError('API did not recover after source restart')
        grants['principals']['a']['enabled'] = False
        write_private(directory / 'grants.json', grants)
        try:
            query('/v1/contracts', 'a')
        except HTTPError as error:
            if error.code != 401:
                raise ValueError('Unexpected local revocation response') from None
        else:
            raise ValueError('Local revocation did not deny the API')
        write_private(directory / 'report.json', {
            'source': 'existing LocalNet Visibility records', 'api': 'Rust REST and GraphQL',
            'scoped_active_counts': {'a': 2, 'b': 2, 'ab': 3}, 'exact_strings_preserved': True,
            'restarted_worker': True, 'local_caller_revocation_denied': True,
            'history': 'snapshot_forward_partial', 'outcomia_live_proof': False,
            'source_outage_denied_and_recovered': True, 'ledger_writes': False})
        print('PASS: actual PQS -> Rust -> owned PostgreSQL -> authenticated REST/GraphQL; A=2 B=2 AB=3; exact strings, pagination, restart and local revocation')
    finally:
        grants['principals']['a']['enabled'] = True
        write_private(directory / 'grants.json', grants)
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
