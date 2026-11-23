"""Verify a private owned-destination backup in a separately provisioned empty LocalNet database."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import select
import time
import shlex
import subprocess
from .common import ROOT, write_private

MAX_FINGERPRINT_BYTES = 128 * 1024 * 1024


def connection(variable):
    """Accept explicit fixture libpq fields; credentials stay in child environment, never argv/output."""
    if not re.fullmatch(r'CANTONGRID_[A-Z0-9_]+', variable):
        raise ValueError('Invalid recovery credential reference')
    fields = dict(part.split('=', 1) for part in shlex.split(os.environ.get(variable, '')))
    if (set(fields) != {'host', 'port', 'user', 'password', 'dbname'} or fields.get('host') != '127.0.0.1'
            or not fields.get('port', '').isdigit()
            or not all(re.fullmatch(r'[A-Za-z][A-Za-z0-9_]{0,62}', fields.get(name, '')) for name in ('user', 'dbname'))):
        raise ValueError('Recovery supports only explicit numeric-loopback fixture connections')
    if not 1 <= int(fields['port']) <= 65535:
        raise ValueError('Invalid recovery port')
    fields['port'] = str(int(fields['port']))
    environment = {key: value for key, value in os.environ.items() if not key.startswith('PG')}
    for key, name in [('HOST', 'host'), ('PORT', 'port'), ('USER', 'user'), ('DATABASE', 'dbname'), ('PASSWORD', 'password')]:
        if name in fields:
            environment['PG' + key] = fields[name]
    environment.update(PGSSLMODE='disable', PGCONNECT_TIMEOUT='3', PGTZ='UTC', PGCLIENTENCODING='UTF8',
                       PGOPTIONS='-c statement_timeout=5000 -c lock_timeout=1000')
    return fields, environment


def sql(environment, statement):
    """Use fixed SQL or catalog-quoted identifiers and keep private failure diagnostics out of reports."""
    result = subprocess.run(['psql', '-X', '-qAt', '-v', 'ON_ERROR_STOP=1'], input=statement,
                            env=environment, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, timeout=30)
    if result.returncode:
        raise ValueError('Recovery database check failed; no credential values are reported')
    return result.stdout.strip()


def check_toolchain(environment):
    """Fail explicitly before dumping when host tools cannot read this server/archive version."""
    server_major = int(sql(environment, 'SHOW server_version_num;')) // 10000
    versions = {}
    for tool in ('pg_dump', 'pg_restore'):
        result = subprocess.run([tool, '--version'], text=True, stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=5)
        match = re.search(r'\(PostgreSQL\) (\d+)\.', result.stdout)
        if result.returncode or not match:
            raise ValueError('Cannot verify PostgreSQL backup tool versions')
        versions[tool] = int(match[1])
    if versions['pg_dump'] < server_major or versions['pg_restore'] < versions['pg_dump']:
        raise ValueError('Backup requires pg_dump at least as new as the server and pg_restore at least as new as pg_dump')


def fingerprint(environment):
    """Hash exact PostgreSQL row text and sequence state; financial values never pass through Python floats."""
    tables = sql(environment, "SELECT quote_ident(n.nspname)||'.'||quote_ident(c.relname) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname IN ('cantongrid','cantongrid_models') AND c.relkind='r' ORDER BY 1").splitlines()
    sequences = sql(environment, "SELECT quote_ident(schemaname)||'.'||quote_ident(sequencename) FROM pg_sequences WHERE schemaname IN ('cantongrid','cantongrid_models') ORDER BY 1").splitlines()
    digest = hashlib.sha256()
    total, rows = 0, 0
    for table in tables:
        digest.update(table.encode() + b'\0')
        process = subprocess.Popen(['psql', '-X', '-qAt', '-v', 'ON_ERROR_STOP=1'], env=environment,
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        process.stdin.write(f'SELECT row_to_json(t)::text FROM {table} t ORDER BY row_to_json(t)::text;'.encode())
        process.stdin.close()
        try:
            deadline = time.monotonic() + 30
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not select.select([process.stdout], [], [], remaining)[0]:
                    raise ValueError('Recovery fingerprint deadline exceeded')
                chunk = process.stdout.read1(65536)
                if not chunk:
                    break
                total += len(chunk)
                if total > MAX_FINGERPRINT_BYTES:
                    raise ValueError('Recovery fingerprint exceeds its local acceptance budget')
                rows += chunk.count(b'\n')
                digest.update(chunk)
            if process.wait(timeout=30):
                raise ValueError('Recovery fingerprint query failed')
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()
            process.stdout.close()
    for sequence in sequences:
        digest.update(sequence.encode() + b'\0')
        digest.update(sql(environment, f'SELECT last_value,is_called FROM {sequence};').encode() + b'\0')
    return {'sha256': digest.hexdigest(), 'tables': len(tables), 'rows': rows, 'sequences': len(sequences)}


def verify(source_variable, restore_variable, output):
    """Restore our own fresh archive only; refuse the original DB, populated targets and unsafe roles."""
    source, source_env = connection(source_variable)
    restore, restore_env = connection(restore_variable)
    if (source['host'], source['port'], source['dbname']) == (restore['host'], restore['port'], restore['dbname']):
        raise ValueError('Restore must use a separate empty database')
    output.mkdir(parents=True, mode=0o700)
    binary = str(Path(os.environ.get('CANTONGRID_BIN', ROOT / 'target/debug/cantongrid')).resolve())
    with (output / 'validation.log').open('w') as log:
        result = subprocess.run([binary, 'storage', 'check', '--database-env', source_variable], stdout=log, stderr=log)
    if result.returncode:
        raise ValueError('Source must be an intact current owned destination')
    safe = sql(restore_env, "SELECT d.datdba=r.oid AND NOT(r.rolsuper OR r.rolcreatedb OR r.rolcreaterole OR r.rolbypassrls OR r.rolreplication) AND NOT EXISTS(SELECT 1 FROM pg_auth_members WHERE member=r.oid) AND d.datname NOT IN ('postgres','template0','template1') FROM pg_database d CROSS JOIN pg_roles r WHERE d.datname=current_database() AND r.rolname=current_user;")
    empty = sql(restore_env, "SELECT NOT EXISTS(SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname NOT LIKE 'pg_%' AND n.nspname<>'information_schema' AND c.relkind IN ('r','p','v','m','f','S'));")
    if safe != 't' or empty != 't':
        raise ValueError('Restore target must be an empty separately owned database under a restricted role')
    check_toolchain(source_env)
    before = fingerprint(source_env)
    archive = output / 'owned-destination.dump'
    # The directory is 0700 and the archive is 0600 before any private record is written.
    descriptor = os.open(archive, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, 'wb') as destination:
        result = subprocess.run(['pg_dump', '-Fc'], env=source_env, stdout=destination,
                                stderr=subprocess.DEVNULL, timeout=120)
    if result.returncode:
        raise ValueError('Owned backup failed')
    if fingerprint(source_env) != before:
        raise ValueError('Source changed during the comparison; retry with workers quiesced')
    result = subprocess.run(['pg_restore', '--exit-on-error', '--single-transaction', '--no-owner',
                             '--dbname', restore['dbname'], str(archive)], env=restore_env,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=120)
    if result.returncode:
        raise ValueError('Isolated restore failed; source database was not changed')
    with (output / 'validation.log').open('a') as log:
        result = subprocess.run([binary, 'storage', 'check', '--database-env', restore_variable], stdout=log, stderr=log)
    if result.returncode or fingerprint(restore_env) != before:
        raise ValueError('Restored schema or exact row/sequence evidence differs from source')
    report = {'format_version': 1, 'scope': 'owned_loopback_backup_restore', 'exact_rows_and_sequences_equal': True,
              'verified_tables': before['tables'], 'verified_rows': before['rows'], 'verified_sequences': before['sequences'],
              'destination_checks_passed': True, 'source_unchanged': True, 'archive_mode': '0600',
              'archive_private': True, 'runtime_key_backup': 'separate_operator_responsibility', 'ledger_writes': False,
              'remote_disaster_recovery_verified': False}
    write_private(output / 'evidence.json', report)
    print(__import__('json').dumps(report, indent=2))


def main(argv=None):
    """Run an explicit local recovery acceptance; never provision roles or drop existing databases."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['verify'])
    parser.add_argument('--source-env', required=True)
    parser.add_argument('--restore-env', required=True)
    parser.add_argument('--output', required=True, type=Path, help='New private output directory')
    args = parser.parse_args(argv)
    verify(args.source_env, args.restore_env, args.output)
