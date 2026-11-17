"""Validate source-release metadata without tagging, publishing or deploying anything."""
import argparse
import os
import re
import tomllib
from .common import ROOT


def check(root=ROOT, tag=None):
    workspace = tomllib.loads((root / 'Cargo.toml').read_text())['workspace']['package']
    version = workspace['version']
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?', version):
        raise ValueError('Invalid release version')
    if workspace.get('license') != 'Apache-2.0' or workspace.get('publish') is not False:
        raise ValueError('Release license/publication metadata differs from the reviewed policy')
    packages = {}
    for manifest in (root / 'crates').glob('*/Cargo.toml'):
        package = tomllib.loads(manifest.read_text())['package']
        if package.get('version') != {'workspace': True} or package.get('license') != {'workspace': True}:
            raise ValueError('A workspace package does not inherit release version/license')
        packages[package['name']] = version
    locked = tomllib.loads((root / 'Cargo.lock').read_text())['package']
    resolved = {p['name']: p['version'] for p in locked if p['name'] in packages and 'source' not in p}
    if not packages or resolved != packages:
        raise ValueError('Lockfile and workspace release versions disagree')
    if f'## {version} ' not in (root / 'CHANGELOG.md').read_text():
        raise ValueError('Release version has no changelog entry')
    if 'Version 2.0, January 2004' not in (root / 'LICENSE').read_text() or not (root / 'NOTICE').is_file():
        raise ValueError('Release license/notice files are missing')
    if tag is not None and tag != f'v{version}':
        raise ValueError('Release tag does not match the workspace version')
    return version


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['check'])
    parser.add_argument('--tag', help='Validate an intended tag without creating it')
    args = parser.parse_args(argv)
    tag = args.tag
    if tag is None and os.environ.get('GITHUB_REF_TYPE') == 'tag':
        tag = os.environ.get('GITHUB_REF_NAME', '')
    print(f'Release metadata verified: {check(tag=tag)} (no publication performed)')
