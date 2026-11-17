"""Security-tool bootstrap rejects tampering, ambiguous archives and external file links."""
import hashlib
import io
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import security


class SecurityBootstrapTests(unittest.TestCase):
    def test_checksums_are_verified_before_cached_execution(self):
        with self.assertRaises(ValueError):
            security.verify(b'changed', hashlib.sha256(b'original').hexdigest())
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            (path / 'example').write_bytes(b'changed')
            with patch.object(security, 'DIRECTORY', path), patch.dict(security.TOOLS, {
                    'example': ('unused', 'unused', hashlib.sha256(b'original').hexdigest())}):
                with self.assertRaises(ValueError):
                    security.binary('example')

    def test_archive_reader_never_extracts_links_or_ambiguous_binaries(self):
        for links in (True, False):
            output = io.BytesIO()
            with tarfile.open(fileobj=output, mode='w:gz') as archive:
                for prefix in ('a', 'b'):
                    member = tarfile.TarInfo(f'{prefix}/example')
                    if links:
                        member.type = tarfile.SYMTYPE
                        member.linkname = '/private'
                        archive.addfile(member)
                    else:
                        member.size = 4
                        archive.addfile(member, io.BytesIO(b'data'))
            with self.assertRaises(ValueError):
                security.extract_binary(output.getvalue(), 'example')

    @patch.object(security.subprocess, 'run')
    @patch.object(security.subprocess, 'check_output', return_value=b'link\0')
    @patch.object(security, 'binary', return_value='synthetic-scanner')
    def test_source_export_refuses_to_follow_tracked_symlinks(self, binary, files, run):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'link').symlink_to('/not-a-repository-file')
            with patch.object(security, 'ROOT', root):
                with self.assertRaises(ValueError):
                    security.secrets()
            self.assertEqual(run.call_count, 1)  # No directory scanner received the external target.


if __name__ == '__main__':
    unittest.main()
