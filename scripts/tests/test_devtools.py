"""Helper regressions with no Docker, HTTP calls, credentials or ledger writes."""
import json
from pathlib import Path
import stat
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools import common, localnet, pqs
from devtools.checks.resources import validate


class DevelopmentToolsTests(unittest.TestCase):
    def test_localnet_logs_are_bounded_without_changing_persistence(self):
        config = {'services': {
            name: {'volumes': ['existing-data:/data'], 'mem_limit': 512,
                   'environment': {'KEEP': 'value'},
                   'ports': [{'target': 5432, 'published': '5440'}]}
            for name in ('canton', 'splice', 'postgres', 'future-service')
        }}
        localnet.apply_local_defaults(config)
        localnet.apply_local_defaults(config)  # Rendering is repeatable.
        validate(config)
        for name, service in config['services'].items():
            self.assertEqual(service['volumes'], ['existing-data:/data'])
            self.assertEqual(service['mem_limit'], 512)
            self.assertEqual(service['environment']['KEEP'], 'value')
            self.assertEqual(service['ports'][0]['host_ip'], '127.0.0.1')
            if name in ('canton', 'splice'):
                self.assertEqual(service['environment']['LOG_LEVEL_STDOUT'], 'INFO')
                self.assertEqual(service['environment']['LOG_LEVEL_CANTON'], 'INFO')

    def test_resource_guard_rejects_missing_and_unbounded_rotation(self):
        for logging in ({}, {'driver': 'json-file', 'options': {'max-file': '3'}},
                        {'driver': 'json-file', 'options': {'max-size': '10m'}},
                        {'driver': 'json-file', 'options': {'max-size': '-1', 'max-file': '3'}}):
            with self.subTest(logging=logging), self.assertRaises(ValueError):
                validate({'services': {'example': {'logging': logging}}})

    def test_root_and_database_profiles_stay_isolated(self):
        self.assertTrue((common.ROOT / 'Cargo.toml').is_file())
        owner = common.pg_command('cantongrid_visibility')
        reader = common.pg_command(reader=True)
        self.assertIn('pqs_owner', owner)
        self.assertIn('cantongrid_pqs_reader', reader)
        self.assertIn('127.0.0.1', reader)
        self.assertNotIn('pqs_owner', reader)
        self.assertIn('cantongrid-visibility', common.compose(True))
        self.assertIn('cantongrid-pqs', common.compose(False))

    def test_private_files_are_restrictive_on_creation_and_rewrite(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / 'nested/source.conf'
            common.write_private(path, {'example': 'synthetic'})
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            path.chmod(0o644)
            common.write_private(path, {'replacement': True})
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
            self.assertEqual(json.loads(path.read_text()), {'replacement': True})

    @patch.object(pqs, 'token', return_value='synthetic-dev-token')
    def test_nested_hocon_profiles_preserve_encoding_and_scope(self, token):
        for subject, parties, metadata, database in [
            ('provider', 'A', '*', 'pqs'), ('ab', '*', '!*', 'cantongrid_visibility')
        ]:
            config = pqs.source_config(subject, parties, metadata, database)
            self.assertEqual(config['source']['ledger']['port'], 3901)
            self.assertEqual(config['pipeline']['filter']['parties'], parties)
            self.assertEqual(config['pipeline']['filter']['metadata'], metadata)
            self.assertEqual(config['target']['postgres']['database'], database)
            self.assertTrue(config['target']['encoding']['numericAsString'])
            self.assertTrue(config['target']['encoding']['int64AsString'])
            self.assertEqual(config['logger']['level'], 'Warning')
            token.assert_called_with(subject, 3600)

    @patch.object(pqs, 'write_private')
    @patch.object(pqs, 'request')
    @patch.object(pqs, 'token', return_value='synthetic-dev-token')
    @patch.object(pqs, 'get')
    def test_existing_broader_source_grants_are_never_repaired_silently(self, get, token, request, write):
        get.side_effect = [
            {'user': {'primaryParty': 'A'}},
            {'user': {'primaryParty': 'A', 'isDeactivated': False}},
            {'rights': [{'kind': {'CanActAs': {'value': {'party': 'A'}}}}]},
        ]
        with self.assertRaises(ValueError):
            pqs.prepare_provider()
        request.assert_not_called()
        write.assert_not_called()

    @patch.object(pqs.subprocess, 'run')
    @patch.object(pqs, 'prepare_visibility')
    @patch.object(pqs, 'wait_source')
    @patch.object(pqs, 'prepare_provider')
    def test_up_recreates_only_the_selected_profile(self, provider, wait, visibility, run):
        pqs.up()
        provider.assert_called_once()
        visibility.assert_not_called()
        self.assertEqual(run.call_count, 2)
        self.assertEqual(run.call_args.args[0][-2:], ['--no-deps', 'pqs'])
        run.reset_mock()
        pqs.up(visibility=True)
        visibility.assert_called_once()
        self.assertEqual(run.call_count, 1)
        self.assertIn('cantongrid-visibility', run.call_args.args[0])
        self.assertNotIn('--volumes', run.call_args.args[0])

    @patch.object(pqs.subprocess, 'run')
    def test_visibility_reader_is_rejected_before_database_access(self, run):
        with self.assertRaises(SystemExit) as error:
            pqs.main(['reader', '--visibility'])
        self.assertEqual(error.exception.code, 2)
        run.assert_not_called()


if __name__ == '__main__':
    unittest.main()
