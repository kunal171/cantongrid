"""Read-only LocalNet policy acceptance; shared operations plumbing owns infrastructure and API lifetime."""
import json
import re
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'examples/poc'))
import support as s  # noqa: E402
from devtools.common import write_private  # noqa: E402


def coefficient(value, scale):
    """Convert decimal text to an exact Python integer; no float or decimal-context rounding is allowed."""
    if not isinstance(value, str) or len(value) > 40 or not re.fullmatch(r'-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?', value):
        raise ValueError('Expected canonical fixed-point source text')
    integer, _, fraction = value.partition('.')
    if len(fraction) > scale:
        raise ValueError('Source precision exceeds the declared model scale')
    # For example 0.0000000001 at scale 10 becomes 1, then multiplying by 10000 stays exact.
    sign = -1 if integer.startswith('-') else 1
    return sign * (int(integer.lstrip('-')) * 10 ** scale + int(fraction.ljust(scale, '0') or '0'))


def lot(value):
    """Accept the Ledger's exact integer/string transports, rejecting booleans, floats and out-of-range values."""
    if isinstance(value, bool) or not isinstance(value, (int, str)) or not re.fullmatch(r'-?(?:0|[1-9][0-9]*)', str(value)):
        raise ValueError('Expected an exact Int64 lot')
    result = int(value)
    if not -(2 ** 63) <= result < 2 ** 63:
        raise ValueError('Lot exceeds Int64 range')
    return result


def policy():
    """Check terms already present in the real PoC model; these are arithmetic checks, not collateral claims."""
    def field(name, kind):
        """Read only a projected field, with its exact required type."""
        return {'op': 'field', 'path': [name], 'as': kind}
    numeric = {'type': 'numeric', 'scale': 10}
    return {'version': 1, 'policy': {'name': 'poc_terms', 'revision': 1, 'fields': {
        'scaled': {'op': 'multiply', 'left': field('priceTick', numeric),
                   'right': {'op': 'literal', 'value': {'type': 'int64', 'value': '10000'}}},
        'bps': field('priceTickBps', numeric), 'lot': field('quantityLot', {'type': 'int64'}),
        'zero': {'op': 'literal', 'value': {'type': 'int64', 'value': '0'}}},
        'checks': [{'name': 'basis_points', 'left': 'scaled', 'operator': 'eq', 'right': 'bps'},
                   {'name': 'positive_lot', 'left': 'lot', 'operator': 'gt', 'right': 'zero'}]}}


def configure(config, directory, namespace, cli, source_rows, boundary):
    """Bind only a new owned namespace/stream, after an independent authorized Ledger inventory comparison."""
    if not 1 <= len(source_rows) <= 128:
        raise ValueError('Reconciliation requires 1–128 selected canonical inputs')
    if len(source_rows) != len({row['contractId'] for row in source_rows}):
        raise ValueError('PQS snapshot contains duplicate contract identities')
    ledger = s.ledger_snapshot(int(boundary))
    if {row['contractId'] for row in ledger} != {row['contractId'] for row in source_rows}:
        raise ValueError('Ledger and PQS inventories differ at the completed indexed boundary')
    by_id = {row['contractId']: row['payload'] for row in ledger}
    for row in source_rows:
        actual, expected = row['payload'], by_id[row['contractId']]
        if coefficient(actual['priceTick'], 10) != coefficient(expected['priceTick'], 10) or lot(actual['quantityLot']) != lot(expected['quantityLot']):
            raise ValueError('Ledger and PQS exact term values differ')
        if coefficient(expected['priceTick'], 10) <= 0 or lot(expected['quantityLot']) <= 0:
            raise ValueError('Existing PoC terms violate their declared Daml ensures')
    models = json.loads((s.DIRECTORY / 'models.json').read_text())
    if len(models['models']) != 1 or models['models'][0]['template'] != config['templates'][0]:
        raise ValueError('Expected the existing exact PoC model/template')
    model = models['models'][0]
    model_path, policy_path = directory / 'models.json', directory / 'policy.json'
    write_private(model_path, models); write_private(policy_path, policy())
    preflight = cli('reconcile', 'preflight', policy_path, model_path, namespace, model['name'])['preflight']
    cli('models', 'register', model_path, '--namespace', namespace, '--database-env', config['destination_database_env'])
    cli('models', 'install', namespace, model['name'], str(model['revision']), '--database-env', config['destination_database_env'])
    cli('models', 'bind', directory / 'runtime.json', namespace, model['name'], str(model['revision']))
    manifest = {'version': 1, 'policies': [{'name': 'poc_terms', 'namespace': namespace,
        'model': model['name'], 'model_revision': model['revision'], 'model_fingerprint': preflight['model']['fingerprint'],
        'policy_file': 'policy.json', 'policy_fingerprint': preflight['policy']['fingerprint']}]}
    write_private(directory / 'policies.json', manifest)
    grants = json.loads((directory / 'grants.json').read_text())
    grants['principals']['narrow'] = {'enabled': True, 'parties': config['authorization']['parties'][:1]}
    if len(config['authorization']['parties']) < 2:
        raise ValueError('The retained AB PoC is required for a genuine narrow-scope control')
    write_private(directory / 'grants.json', grants)
    config['reconciliation_file'] = 'policies.json'
    write_private(directory / 'runtime.json', config)
    return {'grants': grants, 'definition': preflight['fingerprint'], 'manifest': manifest}


def verify(config, directory, source_rows, environment, base, token, context, request, bearer, cli):
    """Compare exact expected summaries and both transports; mutate only the isolated application grant file."""
    endpoint = '/v1/reconciliation/poc_terms/query'
    status, report = request(base, token, endpoint, {})
    if status != 200 or report['comparison'] != 'satisfied' or report['coverage'] != 'retained_indexed_prefix':
        raise ValueError('Real indexed reconciliation did not produce complete satisfied arithmetic checks')
    if report['contracts'] != str(len(source_rows)) or not report['completeWithinConfiguredScope'] or report['definitionFingerprint'] != context['definition']:
        raise ValueError('Reconciliation input count or exact definition identity differs')
    expected = [{'name': name, 'satisfied': str(len(source_rows)), 'violated': '0', 'unavailable': '0'}
                for name in ['basis_points', 'positive_lot']]
    if report['checks'] != expected:
        raise ValueError('Complete exact term-check summaries differ')
    document = (ROOT / 'examples/reconciliation/query.graphql').read_text()
    status, graphql = request(base, token, '/reconciliation/graphql', {'query': document, 'variables': {'name': 'poc_terms'}})
    if status != 200 or graphql.get('errors') or graphql['data']['reconciliation'] != report:
        raise ValueError('Real REST and typed GraphQL reports differ')
    status, metadata = request(base, token, '/v1/reconciliation')
    if status != 200 or len(metadata['policies']) != 1:
        raise ValueError('Configured policy discovery differs')
    narrow = bearer(environment[config['api']['token_key_env']], config['api']['issuer'], config['api']['audience'], 'narrow')
    denied = request(base, narrow, endpoint, {})
    if denied[0] != 403 or request(base, narrow, '/v1/reconciliation/unknown/query', {}) != denied:
        raise ValueError('Narrow-scope denial depends on configured policy existence')
    grants = context['grants']
    try:
        revoked = json.loads(json.dumps(grants)); revoked['epoch'] += 1
        revoked['principals']['reader']['enabled'] = False
        write_private(directory / 'grants.json', revoked)
        if request(base, token, endpoint, {})[0] != 401:
            raise ValueError('Revoked local application principal retained reconciliation access')
    finally:
        write_private(directory / 'grants.json', {**grants, 'epoch': grants['epoch'] + 2})
    if request(base, token, endpoint, {}) != (200, report):
        raise ValueError('Restored local application grants changed the exact indexed report')
    # A nonzero tick times 10001 must differ from the model's times-10000 value, even at its last digit.
    incorrect = policy(); incorrect['policy']['revision'] = 2
    incorrect['policy']['fields']['scaled']['right']['value']['value'] = '10001'
    try:
        write_private(directory / 'policy.json', incorrect)
        validated = cli('reconcile', 'validate', directory / 'policy.json')
        changed = json.loads(json.dumps(context['manifest']))
        changed['policies'][0]['policy_fingerprint'] = validated['policy']['fingerprint']
        write_private(directory / 'policies.json', changed)
        status, violated = request(base, token, endpoint, {})
        if status != 200 or violated['comparison'] != 'violated' or violated['policy']['revision'] != '2':
            raise ValueError('Controlled false condition did not produce a complete violated comparison')
        expected_violations = [{'name': 'basis_points', 'satisfied': '0', 'violated': str(len(source_rows)), 'unavailable': '0'},
                               {'name': 'positive_lot', 'satisfied': str(len(source_rows)), 'violated': '0', 'unavailable': '0'}]
        if violated['checks'] != expected_violations or violated['definitionFingerprint'] == report['definitionFingerprint']:
            raise ValueError('Controlled violation counts or policy identity did not change')
        status, graphql = request(base, token, '/reconciliation/graphql', {'query': document, 'variables': {'name': 'poc_terms'}})
        if status != 200 or graphql.get('errors') or graphql['data']['reconciliation'] != violated:
            raise ValueError('Controlled violation differs between REST and GraphQL')
    finally:
        write_private(directory / 'policy.json', policy())
        write_private(directory / 'policies.json', context['manifest'])
    if request(base, token, endpoint, {}) != (200, report):
        raise ValueError('Restored policy did not restore the complete original report')
    # Whole report is intentionally private: its configured namespace identifies this isolated run.
    write_private(directory / 'reconciliation-report.json', report)
    return {'format_version': 1, 'scope': 'existing_localnet_read_only_ledger',
            'contracts_compared': len(source_rows), 'checks_per_contract': 2,
            'ledger_pqs_inventory_equal': True, 'same_source_boundary': True, 'exact_source_terms_equal': True,
            'complete_retained_indexed_coverage': True, 'rest_graphql_equal': True, 'pinned_definition': True,
            'narrow_scope_denial': True, 'isolated_caller_revocation': True,
            'controlled_violation': True, 'policy_restore_equal': True,
            'ledger_writes': False, 'ledger_grants_changed': False, 'funded_assets_or_trades': False,
            'remote_testnet_verified': False}, report
