"""Validate source-release metadata without tagging, publishing or deploying anything."""
import argparse
import os
import re
import tomllib
from .common import ROOT


def check(root=ROOT, tag=None):
    """Verify version, license and packaged-asset consistency without tagging or publishing."""
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']
    version = workspace['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('Invalid release version')
    if workspace.get('license') != 'Apache-2.0' or workspace.get('publish') != ['crates-io']:
        raise ValueError('Release license/publication metadata differs from the reviewed policy')
    if workspace.get('repository') != 'https://github.com/kunal171/cantongrid':
        raise ValueError('Release repository differs from the project')
    if workspace.get('include') != ['src/**', 'tests/**', 'assets/**', 'migrations/**', 'Cargo.toml', 'README.md', 'LICENSE', 'NOTICE']:
        raise ValueError('Package include policy differs from the reviewed source-only set')
    packages = {}
    for manifest in (root / 'crates').glob('*/Cargo.toml'):
        package = tomllib.loads(manifest.read_text())['package']
        if (package.get('version') != {'workspace': True} or package.get('license') != {'workspace': True}
                or package.get('publish') != {'workspace': True}):
            raise ValueError('A workspace package does not inherit release version/license/publication policy')
        for field in ('repository', 'homepage', 'keywords', 'include'):
            if package.get(field) != {'workspace': True}:
                raise ValueError('A package does not inherit the publication metadata policy')
        if not package.get('description') or package.get('readme') != 'README.md':
            raise ValueError('A package needs a description and its own README')
        packages[package['name']] = version
    locked = tomllib.loads((root / 'Cargo.lock').read_text())['package']
    resolved = {p['name']: p['version'] for p in locked if p['name'] in packages and 'source' not in p}
    if not packages or resolved != packages:
        raise ValueError('Lockfile and workspace release versions disagree')
    if f'## {version} ' not in (root / 'CHANGELOG.md').read_text():
        raise ValueError('Release version has no changelog entry')
    if 'Version 2.0, January 2004' not in (root / 'LICENSE').read_text() or not (root / 'NOTICE').is_file():
        raise ValueError('Release license/notice files are missing')
    for manifest in (root / 'crates').glob('*/Cargo.toml'):
        crate = manifest.parent
        for name in ('LICENSE', 'NOTICE'):
            if (crate / name).read_bytes() != (root / name).read_bytes():
                raise ValueError('Packaged license/notice differs from the repository')
        if not (crate / 'README.md').is_file():
            raise ValueError('Package README is missing')
        # Crate-local fixture copies make unpacked archives testable. The checked-in
        # examples remain canonical; drift must be fixed before any publication.
        for asset in (crate / 'assets').rglob('*'):
            if asset.is_file():
                source = root / 'examples' / asset.relative_to(crate / 'assets')
                if not source.is_file() or asset.read_bytes() != source.read_bytes():
                    raise ValueError('Packaged fixture differs from its canonical example')
    if tag is not None and tag != f'v{version}':
        raise ValueError('Release tag does not match the workspace version')
    return version


def main(argv=None):
    """Validate the intended release tag against source metadata; perform no publication."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['check'])
    parser.add_argument('--tag', help='Validate an intended tag without creating it')
    args = parser.parse_args(argv)
    tag = args.tag
    if tag is None and os.environ.get('GITHUB_REF_TYPE') == 'tag':
        tag = os.environ.get('GITHUB_REF_NAME', '')
    print(f'Release metadata verified: {check(tag=tag)} (no publication performed)')
