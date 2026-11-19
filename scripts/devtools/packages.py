"""Inspect verified Cargo archives; never upload or read registry credentials."""
import argparse
import hashlib
import json
import os
import re
import posixpath
from pathlib import Path
import subprocess
import tarfile
import tomllib
from .common import ROOT
from .release import check


def verify(directory):
    """Verify the allowlisted source files and legal inputs in each expected workspace archive."""
    version = check()
    report = []
    for manifest in sorted((ROOT / 'crates').glob('*/Cargo.toml')):
        package = tomllib.loads(manifest.read_text())['package']['name']
        archive = directory / f'{package}-{version}.crate'
        # Workspace publish verification stages dependencies in Cargo's temporary registry.
        if not archive.is_file():
            archive = directory / 'tmp-registry' / archive.name
        prefix = f'{package}-{version}/'
        with tarfile.open(archive, 'r:gz') as bundle:
            members = bundle.getmembers()
            for member in members:
                relative = member.name.removeprefix(prefix)
                path = Path(relative)
                if (not member.name.startswith(prefix) or path.is_absolute() or '..' in path.parts
                        or not member.isfile() or member.issym() or member.islnk()):
                    raise ValueError('Package archive contains an unsafe entry')
                if not (relative in ('Cargo.toml', 'Cargo.toml.orig', 'Cargo.lock', '.cargo_vcs_info.json',
                                     'README.md', 'LICENSE', 'NOTICE')
                        or path.parts[0] in ('src', 'tests', 'assets', 'migrations')):
                    raise ValueError('Package archive contains a file outside the source allowlist')
            names = {member.name for member in members}
            # Normal package builds do not compile cfg(test) includes. Validate every
            # literal asset reference too, so registry users can run the packaged tests.
            for member in members:
                if not member.name.endswith('.rs'):
                    continue
                text = bundle.extractfile(member).read().decode()
                for include in re.findall(r'include_(?:str|bytes)!\(\s*"([^\"]+)"\s*\)', text):
                    target = posixpath.normpath(posixpath.join(posixpath.dirname(member.name), include))
                    if not target.startswith(prefix) or target not in names:
                        raise ValueError('Package has a missing or external compile-time asset')
            for name in ('LICENSE', 'NOTICE'):
                if bundle.extractfile(prefix + name).read() != (ROOT / name).read_bytes():
                    raise ValueError('Package archive is missing the reviewed legal content')
            metadata = tomllib.loads(bundle.extractfile(prefix + 'Cargo.toml').read().decode())['package']
            if metadata.get('publish') != ['crates-io'] or metadata.get('version') != version:
                raise ValueError('Packaged identity/publication policy differs from the workspace')
            vcs = json.loads(bundle.extractfile(prefix + '.cargo_vcs_info.json').read())
            if not re.fullmatch(r'[0-9a-f]{40}', vcs.get('git', {}).get('sha1', '')):
                raise ValueError('Package archive has no valid source commit')
            if vcs['git'].get('dirty', False):
                raise ValueError('Publishable archives must be built from a clean commit')
        report.append({'name': package, 'version': version, 'bytes': archive.stat().st_size,
                       'sha256': hashlib.sha256(archive.read_bytes()).hexdigest()})
    print(json.dumps({'archives': report, 'uploaded': False}, indent=2))


def main(argv=None):
    """Inspect the selected Cargo archive directory after a dry run; never upload packages."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['verify'])
    parser.add_argument('--directory', type=Path,
                        default=Path(os.environ.get('CARGO_TARGET_DIR', ROOT / 'target')) / 'package')
    args = parser.parse_args(argv)
    verify(args.directory)
