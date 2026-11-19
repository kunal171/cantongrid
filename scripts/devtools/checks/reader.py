"""Verify the fixture SQL reader can query PQS but cannot mutate or become owner."""
import json
import subprocess

from ..common import ENV, pg_command

COMMAND = pg_command(reader=True, sqlstate=True)


def main():
    """Probe SELECT access and rollback-only write/escalation denials for the restricted source reader."""
    sql = '''SELECT json_build_object(
      'reader', current_user = 'cantongrid_pqs_reader',
      'default_read_only', current_setting('default_transaction_read_only') = 'on',
      'can_create', has_schema_privilege(current_user, 'public', 'CREATE'),
      'can_create_schema', has_database_privilege(current_user, 'pqs', 'CREATE'),
      'can_create_temp', has_database_privilege(current_user, 'pqs', 'TEMP'),
      'can_write', (SELECT bool_or(has_table_privilege(current_user, c.oid,
          'INSERT,UPDATE,DELETE,TRUNCATE,REFERENCES,TRIGGER'))
          FROM pg_class c WHERE relnamespace = 'public'::regnamespace AND relkind IN ('r','p')),
      'active_rows', (SELECT count(*) FROM active())
    );'''
    result = subprocess.run(COMMAND, input=sql, text=True, capture_output=True, check=True, env=ENV)
    data = json.loads(result.stdout)
    if (not data['reader'] or not data['default_read_only'] or data['active_rows'] < 1
            or any(data[key] for key in ('can_create', 'can_create_schema', 'can_create_temp', 'can_write'))):
        raise ValueError('Reader privilege inspection failed')
    cases = {
        'data write': 'DELETE FROM public.__watermark WHERE false;',
        'schema write': 'CREATE TABLE public.cantongrid_reader_write_probe (id bigint);',
        'temporary table': 'CREATE TEMP TABLE cantongrid_reader_temp_probe (id bigint);',
        'owner escalation': 'SET ROLE pqs_owner;',
    }
    for label, statement in cases.items():
        # Disable the default read-only setting: real ACLs must still deny writes.
        # Zero-row DML and rollback keep this harmless even if a privilege regresses.
        result = subprocess.run(COMMAND, input='BEGIN; SET TRANSACTION READ WRITE;\n' + statement + '\nROLLBACK;\n',
                                text=True, capture_output=True, env=ENV)
        if result.returncode == 0 or '42501' not in result.stderr:
            raise ValueError(f'Expected insufficient-privilege rejection for {label}')
        print(f'PASS: {label} denied by database permissions')
    print('PASS: authenticated SQL reader can query active data without source ownership or write privileges')
