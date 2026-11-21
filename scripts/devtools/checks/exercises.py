"""M0 catalog-only inspection of the retained schema041 fixture; no Ledger event reads."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from ..common import ENV, ROOT, pg_command, write_private

MAX_BYTES = 512 * 1024
SQL = r"""
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout = '5s';
SELECT json_build_object(
 'schema_version',(SELECT version FROM public.flyway_schema_history WHERE success AND version ~ '^[0-9]+$' ORDER BY installed_rank DESC LIMIT 1),
 'read_only',current_setting('transaction_read_only')='on',
 'restricted_reader',(SELECT NOT r.rolsuper AND NOT r.rolcreatedb AND NOT r.rolcreaterole AND NOT r.rolreplication AND NOT r.rolbypassrls
   AND d.datdba<>r.oid AND NOT EXISTS(SELECT 1 FROM pg_auth_members WHERE member=r.oid)
   AND NOT has_database_privilege(current_user,current_database(),'CREATE,TEMP')
   AND NOT has_schema_privilege(current_user,'public','CREATE')
   AND NOT EXISTS(SELECT 1 FROM pg_proc WHERE pronamespace='public'::regnamespace AND prosecdef)
   AND NOT EXISTS(SELECT 1 FROM pg_class c WHERE c.relnamespace='public'::regnamespace AND c.relkind IN ('r','p')
     AND has_table_privilege(current_user,c.oid,'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER'))
   FROM pg_roles r JOIN pg_database d ON d.datname=current_database() WHERE r.rolname=current_user),
 'functions',COALESCE((SELECT json_agg(row_to_json(f)) FROM (
   SELECT p.proname AS name,pg_get_function_identity_arguments(p.oid) AS arguments,
     pg_get_function_result(p.oid) AS result,p.prosecdef AS security_definer,CASE WHEN octet_length(pg_get_functiondef(p.oid))<=16384 THEN pg_get_functiondef(p.oid) END AS definition
   FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
   WHERE n.nspname='public' AND p.proname IN ('exercises','__exercises','creates','archives','active','oldest_offset','latest_offset','pruned_offset')
   ORDER BY p.proname,pg_get_function_identity_arguments(p.oid) LIMIT 32
 ) f),'[]'::json),
 'attributes',COALESCE((SELECT json_agg(row_to_json(a)) FROM (
   SELECT c.relname AS type,a.attname AS name,format_type(a.atttypid,a.atttypmod) AS sql_type,a.attnum AS position
   FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
   WHERE n.nspname='public' AND c.relname IN ('exercise','contract','event_id') AND a.attnum>0 AND NOT a.attisdropped
   ORDER BY c.relname,a.attnum LIMIT 128
 ) a),'[]'::json),
 'columns',COALESCE((SELECT json_agg(row_to_json(a)) FROM (
   SELECT c.relname AS relation,a.attname AS name,format_type(a.atttypid,a.atttypmod) AS sql_type,a.attnotnull AS not_null
   FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace
   WHERE n.nspname='public' AND (c.relname IN ('__events','__transactions','__contracts','__exercises','__exercise_tpe') OR c.relname LIKE '%redact%')
     AND a.attnum>0 AND NOT a.attisdropped
   ORDER BY c.relname,a.attnum LIMIT 256
 ) a),'[]'::json)
);
ROLLBACK;
"""


def inspect_catalog(database='pqs'):
    """Use the existing dedicated SELECT-only reader; never provision roles or invoke event functions."""
    result = subprocess.run(pg_command(database, reader=True), input=SQL, text=True,
                            capture_output=True, check=True, timeout=15, env=ENV)
    if len(result.stdout.encode()) > MAX_BYTES:
        raise ValueError('Catalog evidence exceeds the supported byte bound')
    try:
        data = json.loads(result.stdout)
    except (ValueError, TypeError) as exc:
        raise ValueError('Unsupported catalog response') from exc
    if (not isinstance(data, dict) or data.get('schema_version') != '041'
            or data.get('read_only') is not True or data.get('restricted_reader') is not True):
        raise ValueError('Exercise inspection requires schema041 and a restricted read-only snapshot')
    for name, cap in [('functions', 32), ('attributes', 128), ('columns', 256)]:
        items = data.get(name)
        if not isinstance(items, list) or len(items) >= cap or any(not isinstance(item, dict) for item in items):
            raise ValueError('Catalog collection is invalid or reaches its evidence bound')
    if any(not isinstance(item.get('definition'), str) for item in data['functions']):
        raise ValueError('Catalog function definition is unavailable or oversized')
    return data


def summarize(data):
    """Publish schema facts only. No transaction IDs, private party lists, rows or arguments are queried."""
    functions = data.get('functions', [])
    attributes = data.get('attributes', [])
    if not isinstance(functions, list) or not isinstance(attributes, list):
        raise ValueError('Unsupported catalog collections')
    exercise = [item for item in attributes if item.get('type') == 'exercise']
    return {
        'evidence_kind': 'retained_database_catalog_only',
        'schema_version': data['schema_version'],
        'restricted_reader': True,
        'exercise_overloads': [{'arguments': item['arguments'], 'result': item['result']}
                               for item in functions if item.get('name') == 'exercises'],
        'exercise_attributes': [{'name': item['name'], 'sql_type': item['sql_type']} for item in exercise],
        'event_id_attributes': [{'name': item['name'], 'sql_type': item['sql_type']}
                                for item in attributes if item.get('type') == 'event_id'],
        'function_definitions_sha256': hashlib.sha256(json.dumps(functions, sort_keys=True).encode()).hexdigest(),
        'live_authorization_verified': False,
        'exercise_visibility_verified': False,
        'lineage_enabled': False,
    }


def test_fixture(dpm):
    """Build/test a disposable copy using the SDK launcher, without contacting an external Ledger.

    DPM 1.1.0 percent-encodes spaces in local DAR dependency paths. Copying only
    source inputs to /tmp avoids that path issue and leaves the SDK cache intact.
    The SDK manifest, not a guessed damlc path, selects the script-service runner.
    """
    executable = shutil.which(dpm)
    if executable is None:
        raise ValueError('DPM is unavailable; pass --dpm with its installed executable path')
    with tempfile.TemporaryDirectory(prefix='cantongrid-m0-', dir='/tmp') as directory:
        target = Path(directory) / 'exercises'
        shutil.copytree(ROOT / 'examples/exercises', target,
                        ignore=shutil.ignore_patterns('.daml'))
        for package, command in [('contracts', 'build'), ('tests', 'test')]:
            subprocess.run([executable, command], cwd=target / package,
                           check=True, timeout=180, env=ENV)


def main(argv=None):
    """Save owner-only evidence and print a redacted catalog summary; no source/ledger setup occurs."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture-tests', action='store_true',
                        help='Build/test generic Daml fixture on its IDE ledger; no PQS or external Ledger reads/writes')
    parser.add_argument('--dpm', default='dpm', help='Installed SDK launcher for --fixture-tests')
    args = parser.parse_args(argv)
    if args.fixture_tests:
        test_fixture(args.dpm)
        return
    data = inspect_catalog()
    summary = summarize(data)
    write_private(ROOT / '.local/exercise-source/catalog.json', data)
    write_private(ROOT / '.local/exercise-source/catalog-summary.json', summary)
    print(json.dumps(summary, indent=2))
