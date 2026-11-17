"""Pinned local/CI security tools; no runtime dependencies or private service credentials."""
import argparse
import hashlib
import io
from pathlib import Path
import platform
import subprocess
import tarfile
import tempfile
from urllib.request import urlopen
from .common import ROOT

DIRECTORY = ROOT / '.local/security-tools'
# Archive and executable SHA-256 values verified from the upstream release assets.
TOOLS = {
    'gitleaks': (
        'https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz',
        '551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb',
        '88f91962aa2f93ac6ab281d553b9e125f5197bbbce38f9f2437f7299c32e5509'),
    'actionlint': (
        'https://github.com/rhysd/actionlint/releases/download/v1.7.12/actionlint_1.7.12_linux_amd64.tar.gz',
        '8aca8db96f1b94770f1b0d72b6dddcb1ebb8123cb3712530b08cc387b349a3d8',
        'c872d6db8c6bf83a8eaa704fc93999f027d55dffbc63b8a6abdccb47df5f4cd4'),
    'cargo-audit': (
        'https://github.com/rustsec/rustsec/releases/download/cargo-audit/v0.23.0/cargo-audit-x86_64-unknown-linux-gnu-v0.23.0.tgz',
        'b77993532b253fcbf550996d7716465ec3d734d4e74e7f87bfb65a19add0fb59',
        'bff43720181d3927b7907cdb4f32026dd1d1f032b802278cab102f382d4d8ddc'),
    'cargo-deny': (
        'https://github.com/EmbarkStudios/cargo-deny/releases/download/0.21.0/cargo-deny-0.21.0-x86_64-unknown-linux-musl.tar.gz',
        'a2170b854ff697e79e39c4c538981c9c1bdd803addbae6719a4cb66b4585a903',
        '33c3aeb3c0e154060ee6784ae4fb38dd52d51454138f0596ed86252a8b84375b'),
}


def verify(data, expected):
    if hashlib.sha256(data).hexdigest() != expected:
        raise ValueError('Security tool checksum mismatch; refusing installation/execution')


def extract_binary(archive, name):
    # Read only the selected regular file; never extract archive paths or links into the workspace.
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:gz') as source:
        matches = [member for member in source.getmembers()
                   if member.isfile() and Path(member.name).name == name]
        if len(matches) != 1 or matches[0].size > 64 * 1024 * 1024:
            raise ValueError('Unexpected security tool archive contents')
        return source.extractfile(matches[0]).read()


def binary(name):
    path = DIRECTORY / name
    if not path.is_file():
        raise ValueError('Install the pinned security tools before checking')
    verify(path.read_bytes(), TOOLS[name][2])
    return str(path)


def install():
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'amd64'):
        raise ValueError('Pinned binary helper supports Linux x86_64; see contributing security commands')
    DIRECTORY.mkdir(parents=True, exist_ok=True)
    for name, (url, archive_hash, executable_hash) in TOOLS.items():
        path = DIRECTORY / name
        if path.is_file() and hashlib.sha256(path.read_bytes()).hexdigest() == executable_hash:
            continue
        with urlopen(url, timeout=60) as response:
            archive = response.read(64 * 1024 * 1024 + 1)
        verify(archive, archive_hash)
        executable = extract_binary(archive, name)
        verify(executable, executable_hash)
        path.write_bytes(executable)
        path.chmod(0o755)
    print('Pinned security tools verified')


def secrets():
    # Explicit defaults prevent local GITLEAKS_CONFIG or committed ignore files from silently
    # changing CI's coverage. Scoped exceptions require a reviewed policy change here.
    with tempfile.TemporaryDirectory(prefix='cantongrid-secret-check-') as directory:
        root = Path(directory)
        config = root / 'default.toml'
        config.write_text('[extend]\nuseDefault = true\n')
        ignore = root / 'empty.ignore'
        ignore.write_text('')
        command = [binary('gitleaks'), '--redact', '--no-banner', '--ignore-gitleaks-allow',
                   '--config', str(config), '--gitleaks-ignore-path', str(ignore)]
        subprocess.run([*command, 'git', '--log-opts=--all'], cwd=ROOT, check=True)
        files = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=ROOT)
        destination = root / 'eligible-tree'
        destination.mkdir()
        for filename in set(files.decode().split('\0')) - {''}:
            source = ROOT / filename
            if source.is_symlink():
                raise ValueError('Review tracked symlinks before scanning; refusing to follow external files')
            if source.is_file():
                target = destination / filename
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(source.read_bytes())
        subprocess.run([*command, 'dir', str(destination)], cwd=ROOT, check=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['install', 'check'])
    parser.add_argument('area', choices=['all', 'audit', 'dependencies', 'secrets', 'workflows'], nargs='?', default='all')
    args = parser.parse_args(argv)
    if args.command == 'install':
        install()
        return
    areas = ['audit', 'dependencies', 'secrets', 'workflows'] if args.area == 'all' else [args.area]
    for area in areas:
        if area == 'audit':
            subprocess.run([binary('cargo-audit'), 'audit', '--deny', 'warnings'], cwd=ROOT, check=True)
        elif area == 'dependencies':
            subprocess.run([binary('cargo-deny'), 'check', 'licenses', 'bans', 'sources'], cwd=ROOT, check=True)
        elif area == 'workflows':
            subprocess.run([binary('actionlint')], cwd=ROOT, check=True)
        else:
            secrets()
