"""Recovery connection and destination guards run without a database or credential output."""
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import recovery


class RecoveryTests(unittest.TestCase):
    def test_connections_reject_remote_fallback_options_and_normalize_physical_port(self):
        good = 'host=127.0.0.1 port=05442 user=owner password=private-sentinel dbname=owned'
        with patch.dict(os.environ, {'CANTONGRID_RECOVERY_TEST': good, 'PGSERVICE': 'untrusted', 'PGHOST': 'remote'}):
            fields, environment = recovery.connection('CANTONGRID_RECOVERY_TEST')
            self.assertEqual(fields['port'], '5442')
            self.assertEqual(environment['PGHOST'], '127.0.0.1')
            self.assertNotIn('PGSERVICE', environment)
        for value in [good.replace('127.0.0.1', 'remote.example'), good + ' options=-cunsafe',
                      good.replace('dbname=owned', 'dbname=postgresql://remote'),
                      good.replace('port=05442', 'port=0'), good.replace(' password=private-sentinel', '')]:
            with patch.dict(os.environ, {'CANTONGRID_RECOVERY_TEST': value}):
                with self.assertRaises(ValueError) as failed:
                    recovery.connection('CANTONGRID_RECOVERY_TEST')
                self.assertNotIn('private-sentinel', str(failed.exception))

    def test_same_database_is_rejected_before_creating_any_backup_directory(self):
        first = 'host=127.0.0.1 port=5442 user=owner password=private-sentinel dbname=owned'
        with tempfile.TemporaryDirectory() as directory, patch.dict(os.environ, {
                'CANTONGRID_RECOVERY_SOURCE': first,
                'CANTONGRID_RECOVERY_TARGET': first.replace('5442', '05442')}):
            output = Path(directory) / 'must-not-exist'
            with self.assertRaises(ValueError):
                recovery.verify('CANTONGRID_RECOVERY_SOURCE', 'CANTONGRID_RECOVERY_TARGET', output)
            self.assertFalse(output.exists())
