#!/usr/bin/env python3
"""Reproduce the PQS V001 type-namespace collision without Canton or application code."""
import argparse
from pathlib import Path
import subprocess
import tempfile
import time
import uuid
from zipfile import ZipFile

IMAGE = ('europe-docker.pkg.dev/da-images/public/docker/scribe@'
         'sha256:072faffbee3640448b282fc973b04ec99f5ba2bf92d2b43d01a00c89626c3a67')
RESOURCE = 'db/migration/V001__Create_initial_schema.sql'
GUARD = "select 1 from pg_type where typname = '__event_type'"


def call(args, **kwargs):
    """Fail immediately on a reproduction command error; use argument arrays rather than a shell."""
    return subprocess.run(args, check=True, **kwargs)


def main():
    """Compare clean/collision SQL controls in isolated containers and clean up only their owned resources."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--migration-file', type=Path, help='Test the exact SQL from an upstream checkout')
    args = parser.parse_args()
    suffix = uuid.uuid4().hex[:10]
    database = 'cg-schema-repro-' + suffix
    extraction = database + '-image'
    try:
        if args.migration_file:
            migration = args.migration_file.read_text()
        else:
            with tempfile.TemporaryDirectory(prefix='pqs-schema-repro-') as directory:
                call(['docker', 'create', '--name', extraction, IMAGE], capture_output=True)
                jar = Path(directory) / 'scribe.jar'
                call(['docker', 'cp', extraction + ':/scribe.jar', str(jar)], capture_output=True)
                with ZipFile(jar) as archive:
                    migration = archive.read(RESOURCE).decode()
        if migration.count(GUARD) != 1:
            raise ValueError('Expected unscoped guard is absent or ambiguous; re-evaluate this reproduction')
        call(['docker', 'run', '-d', '--name', database, '--network', 'none', '--memory', '512m',
              '-e', 'POSTGRES_USER=repro_owner', '-e', 'POSTGRES_PASSWORD=repro-only',
              '-e', 'POSTGRES_DB=repro', 'postgres:14'], capture_output=True)
        for attempt in range(30):
            if subprocess.run(['docker', 'exec', database, 'pg_isready', '-U', 'repro_owner', '-d', 'repro'],
                              capture_output=True).returncode == 0:
                break
            time.sleep(1)
        else:
            raise RuntimeError('Isolated PostgreSQL did not become ready')
        version = subprocess.check_output(['docker', 'exec', database, 'psql', '-X', '-At',
                                           '-U', 'repro_owner', '-d', 'repro',
                                           '-c', "SELECT current_setting('server_version')"], text=True).strip()
        print('PostgreSQL:', version)
        patched = migration.replace(GUARD, GUARD + ' and typnamespace = current_schema()::regnamespace')
        for label, sql, collision, expect_success in [
            ('original clean schema', migration, False, True),
            ('original with unrelated type', migration, True, False),
            ('scoped guard clean schema', patched, False, True),
            ('scoped guard with unrelated type', patched, True, True),
        ]:
            setup = 'BEGIN;\n'
            if collision:
                setup += "CREATE SCHEMA unrelated; CREATE TYPE unrelated.__event_type AS ENUM ('unrelated');\n"
            setup += 'CREATE SCHEMA target; SET LOCAL search_path TO target;\n'
            result = subprocess.run([
                'docker', 'exec', '-i', database, 'psql', '-X', '-q', '-U', 'repro_owner', '-d', 'repro',
                '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose'],
                input=setup + sql + '\nROLLBACK;\n', text=True, capture_output=True)
            success = result.returncode == 0
            if success != expect_success:
                raise RuntimeError(f'{label}: unexpected result\n{result.stderr[:1500]}')
            if not success and ('42704' not in result.stderr or '__payload_type' not in result.stderr):
                raise RuntimeError('Failure was not the expected missing __payload_type error')
            print('PASS:', label, '(expected missing-type failure)' if not success else '')
        print('Confirmed: namespace-blind type existence check; candidate scoped predicate passes both controls')
        print('Candidate is an in-memory experiment, not an applied Flyway migration patch')
    finally:
        # Only these uniquely named, disposable containers and anonymous volumes are removed.
        subprocess.run(['docker', 'rm', '-fv', database, extraction], capture_output=True)


if __name__ == '__main__':
    main()
