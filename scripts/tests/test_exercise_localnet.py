"""LocalNet runner safety and paging controls use synthetic responses only."""
import contextlib
import importlib
import io
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
localnet = importlib.import_module('devtools.checks.exercise_localnet')
command = importlib.import_module('devtools.checks.exercises')


class ExerciseLocalnetTests(unittest.TestCase):
    """No runner side effects are allowed without its explicit LocalNet opt-in."""

    def test_localnet_requires_explicit_opt_in_before_any_side_effect(self):
        with patch.object(localnet, 'run') as run, contextlib.redirect_stderr(io.StringIO()):
            for args in [['--localnet'], ['--allow-ledger-writes'], ['--resume', 'cg-m0-12345678']]:
                with self.assertRaises(SystemExit):
                    command.main(args)
            run.assert_not_called()

    def test_exporter_has_bounded_logs_and_no_unscoped_party_filter(self):
        config = localnet.exporter_config(Path('/tmp/owned'))
        service = config['services']['probe']
        self.assertEqual(service['logging']['options'], {'max-size': '10m', 'max-file': '3'})
        self.assertEqual(service['ports'], ['127.0.0.1:18083:8080'])
        self.assertEqual(set(localnet.event_format(['a'])['filtersByParty']), {'a'})

    def test_current_source_grants_fail_closed_without_repair(self):
        with patch.object(localnet, 'get', side_effect=[{'user': {'isDeactivated': False}},
               {'rights': [localnet.right('CanActAs', 'a')]}]):
            with self.assertRaises(ValueError):
                localnet.verify_scope('source', ['a'])

    def test_same_offset_page_and_overlap_use_reader_without_dropping_receipts(self):
        row = {'exercise_event_id': {'offset': 9007199254740993, 'node_id': 0}}
        raw = json.dumps(row)
        with patch.object(localnet, 'sql', side_effect=[raw, raw, '']) as query:
            report = localnet.probe_paging('owned', 'reader', 0, 9007199254740994, [row])
        self.assertTrue(report['overlap_exact'])
        self.assertTrue(all(call.args[2] == 'reader' for call in query.call_args_list))
        with patch.object(localnet, 'sql', side_effect=[raw, raw, '']):
            with self.assertRaises(ValueError):
                localnet.probe_paging('owned', 'reader', 0, 9007199254740994, [])

    def test_transaction_binding_rejects_wrong_update_id(self):
        ledger = {'control': {'transaction': {'offset': 123, 'updateId': 'expected'}}}
        with patch.object(localnet, 'sql', return_value='[{"offset":123,"transaction_id":"expected"}]'):
            self.assertTrue(localnet.probe_transaction_ids('owned', 'reader', 0, 123, ledger)['update_ids_match'])
        with patch.object(localnet, 'sql', return_value='[{"offset":123,"transaction_id":"wrong"}]'):
            with self.assertRaises(ValueError):
                localnet.probe_transaction_ids('owned', 'reader', 0, 123, ledger)
