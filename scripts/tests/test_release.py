"""A release guard must reject a wrong version/tag before anything can be published."""
from pathlib import Path
import sys
import shutil
import re
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import release


class ReleaseMetadataTests(unittest.TestCase):
    def test_real_workspace_and_intended_tag_are_consistent(self):
        version = release.check()
        self.assertEqual(release.check(tag='v' + version), version)
        with self.assertRaises(ValueError):
            release.check(tag='v0.0.0-wrong')

    def test_mismatched_lockfile_and_missing_license_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'crates/example').mkdir(parents=True)
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="0.1.0-alpha.1"\nlicense="Apache-2.0"\npublish=["crates-io"]\nrepository="https://github.com/kunal171/cantongrid"\ninclude=["src/**","tests/**","assets/**","migrations/**","Cargo.toml","README.md","LICENSE","NOTICE"]\n')
            (root / 'crates/example/Cargo.toml').write_text('[package]\nname="example"\nversion.workspace=true\nlicense.workspace=true\npublish.workspace=true\nrepository.workspace=true\nhomepage.workspace=true\nkeywords.workspace=true\ninclude.workspace=true\nreadme="README.md"\ndescription="Synthetic release test"\n')
            (root / 'Cargo.lock').write_text('version=4\n[[package]]\nname="example"\nversion="0.1.0-wrong"\n')
            with self.assertRaises(ValueError):
                release.check(root)
            (root / 'Cargo.lock').write_text('version=4\n[[package]]\nname="example"\nversion="0.1.0-alpha.1"\n')
            (root / 'CHANGELOG.md').write_text('## 0.1.0-alpha.1 — unreleased\n')
            (root / 'NOTICE').write_text('Synthetic test notice')
            with self.assertRaises(FileNotFoundError):
                release.check(root)  # LICENSE is deliberately absent after all other metadata matches.

    def test_private_registry_license_and_stale_fixture_changes_fail_closed(self):
        # Use one real package's metadata, without source/build outputs or credentials.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = Path(__file__).resolve().parents[2]
            crate = root / 'crates/core'
            crate.mkdir(parents=True)
            manifest = (project / 'Cargo.toml').read_text()
            (root / 'Cargo.toml').write_text(re.sub(r'^members = .*$', 'members = ["crates/core"]', manifest, flags=re.M))
            for name in ('Cargo.lock', 'CHANGELOG.md', 'LICENSE', 'NOTICE'):
                shutil.copyfile(project / name, root / name)
            for name in ('Cargo.toml', 'README.md', 'LICENSE', 'NOTICE'):
                shutil.copyfile(project / 'crates/core' / name, crate / name)
            source = root / 'examples/fixtures/control.json'
            source.parent.mkdir(parents=True)
            source.write_text('{"synthetic":true}')
            asset = crate / 'assets/fixtures/control.json'
            asset.parent.mkdir(parents=True)
            shutil.copyfile(source, asset)
            release.check(root)
            (root / 'Cargo.toml').write_text(manifest.replace('["crates-io"]', '["another-registry"]'))
            with self.assertRaises(ValueError):
                release.check(root)
            (root / 'Cargo.toml').write_text(manifest)
            (crate / 'NOTICE').write_text('changed notice')
            with self.assertRaises(ValueError):
                release.check(root)
            shutil.copyfile(root / 'NOTICE', crate / 'NOTICE')
            asset.write_text('{"synthetic":false}')
            with self.assertRaises(ValueError):
                release.check(root)
