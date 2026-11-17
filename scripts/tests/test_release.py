"""A release guard must reject a wrong version/tag before anything can be published."""
from pathlib import Path
import sys
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
            (root / 'Cargo.toml').write_text('[workspace.package]\nversion="0.1.0-alpha.1"\nlicense="Apache-2.0"\npublish=false\n')
            (root / 'crates/example/Cargo.toml').write_text('[package]\nname="example"\nversion.workspace=true\nlicense.workspace=true\n')
            (root / 'Cargo.lock').write_text('version=4\n[[package]]\nname="example"\nversion="0.1.0-wrong"\n')
            with self.assertRaises(ValueError):
                release.check(root)
