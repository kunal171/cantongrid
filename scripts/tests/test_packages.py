"""Archive controls must reject missing resources, leaked files and dirty provenance."""
import io
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import packages


class PackageArchiveTests(unittest.TestCase):
    def verify(self, change=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            crate = root / 'crates/example'
            crate.mkdir(parents=True)
            (crate / 'Cargo.toml').write_text('[package]\nname="example"\n')
            (root / 'LICENSE').write_text('Synthetic license')
            (root / 'NOTICE').write_text('Synthetic notice')
            entries = {
                'Cargo.toml': '[package]\nname="example"\nversion="1.0.0"\npublish=["crates-io"]\nreadme="README.md"\n',
                'README.md': '# Example\n\nSynthetic package usage.\n',
                'LICENSE': 'Synthetic license', 'NOTICE': 'Synthetic notice',
                '.cargo_vcs_info.json': json.dumps({'git': {'sha1': 'a' * 40}}),
                'src/lib.rs': '#[cfg(test)] const VALUE: &str = include_str!("../assets/value.json");',
                'assets/value.json': '{"synthetic":true}',
            }
            if change:
                change(entries)
            archive_dir = root / 'package/tmp-registry'
            archive_dir.mkdir(parents=True)
            with tarfile.open(archive_dir / 'example-1.0.0.crate', 'w:gz') as archive:
                for name, text in entries.items():
                    content = text.encode()
                    member = tarfile.TarInfo('example-1.0.0/' + name)
                    member.size = len(content)
                    archive.addfile(member, io.BytesIO(content))
            with patch.object(packages, 'ROOT', root), patch.object(packages, 'check', return_value='1.0.0'), patch('builtins.print'):
                packages.verify(root / 'package')

    def test_clean_workspace_staging_archive_is_accepted(self):
        self.verify()  # Cargo omits dirty=false rather than serializing it explicitly.

    def test_bad_archive_contents_and_provenance_are_rejected(self):
        changes = [
            lambda files: files.pop('README.md'),
            lambda files: files.update({'README.md': ' \n'}),
            lambda files: files.update({'Cargo.toml': files['Cargo.toml'].replace('readme="README.md"', 'readme=false')}),
            lambda files: files.pop('assets/value.json'),
            lambda files: files.update({'../outside': 'bad'}),
            lambda files: files.update({'.env': 'PRIVATE-DO-NOT-PUBLISH'}),
            lambda files: files.update({'NOTICE': 'changed notice'}),
            lambda files: files.update({'.cargo_vcs_info.json': json.dumps({'git': {'sha1': 'a' * 40, 'dirty': True}})}),
        ]
        for change in changes:
            with self.subTest(change=changes.index(change)), self.assertRaises(ValueError):
                self.verify(change)
