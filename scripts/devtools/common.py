"""Shared local Docker/PostgreSQL plumbing and owner-only generated files."""
import json
import os
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ENV = {**os.environ, 'PQS_UID': str(os.getuid()), 'PQS_GID': str(os.getgid())}


def compose(visibility=False):
    project = 'cantongrid-visibility' if visibility else 'cantongrid-pqs'
    filename = 'visibility.compose.yaml' if visibility else 'pqs.compose.yaml'
    return ['docker', 'compose', '--progress', 'plain', '-p', project,
            '-f', str(ROOT / 'dev' / filename)]


def pg_command(database='pqs', *, reader=False, sqlstate=False, tuples=True):
    command = [*compose(), 'exec', '-T']
    if reader:
        command += ['-e', 'PGPASSWORD=local-pqs-reader']  # Disposable fixture password.
    command += ['pqs-postgres', 'psql', '-X', '-qAt' if tuples else '-q']
    if reader:
        command += ['-h', '127.0.0.1']
    command += ['-U', 'cantongrid_pqs_reader' if reader else 'pqs_owner',
                '-d', database, '-v', 'ON_ERROR_STOP=1']
    if sqlstate:
        command += ['-v', 'VERBOSITY=sqlstate']
    return command


def write_private(path, value):
    """Create restrictive permissions before writing; retain them on rewrites."""
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    os.fchmod(fd, 0o600)
    with os.fdopen(fd, 'w') as output:
        output.write(json.dumps(value, indent=2) + '\n')


def source_ready(visibility=False):
    from urllib.request import urlopen
    port = 18081 if visibility else 18080
    try:
        with urlopen(f'http://127.0.0.1:{port}/readyz', timeout=3) as response:
            data = json.load(response)
        return data.get('status') == 'ok' and all(data.get(key) is True for key in
                                                ('grpc_up', 'jdbc_connection_pool_up', 'stream_up'))
    except (OSError, ValueError):
        return False


def wait_source(visibility=False, timeout=45):
    import time
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if source_ready(visibility):
            return
        time.sleep(1)
    raise ValueError('PQS source did not become ready within the startup window')
