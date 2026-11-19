"""Failure controls for application acceptance; no Docker, HTTP, Ledger writes or credentials."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from devtools.checks.outcomia_models import Probe, created


class OutcomiaModelsTests(unittest.TestCase):
    def test_unavailable_exporter_stops_acceptance_before_dar_or_ledger_writes(self):
        from devtools.checks import outcomia_pipeline
        with patch.object(outcomia_pipeline, 'source_ready', return_value=False), \
                patch.object(outcomia_pipeline, 'request') as request:
            with self.assertRaisesRegex(ValueError, 'renew its local credentials'):
                outcomia_pipeline.main(['--dar', 'not-an-existing-file.dar', '--allow-ledger-writes'])
            request.assert_not_called()

    def test_created_contract_must_match_the_pinned_package_and_be_unambiguous(self):
        package = 'a' * 64
        event = {'templateId': package + ':Outcomia.Binary:BinaryMarket', 'contractId': 'owned'}
        self.assertEqual(created([event], package, 'BinaryMarket'), 'owned')
        for events in [[], [event, event], [{**event, 'templateId': 'b' * 64 + ':Outcomia.Binary:BinaryMarket'}]]:
            with self.assertRaises(ValueError):
                created(events, package, 'BinaryMarket')

    def test_optional_field_promotion_restores_after_assertion_or_ambiguous_commit(self):
        for ambiguous in (False, True):
            with self.subTest(ambiguous=ambiguous), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                original = {'version': 1, 'models': [{'revision': 1, 'fields': {}}]}
                (root / 'models.json').write_text(json.dumps(original))
                probe = Probe(Path('unused'), root, {}, lambda *args: None, 'owned')
                calls = []

                def cli(*args):
                    calls.append(args)
                    if ambiguous and args[:2] == ('models', 'bind') and args[5] == 2:
                        raise ValueError('ambiguous promotion')

                with patch.object(probe, 'cli', side_effect=cli), patch.object(probe, 'metadata'):
                    with self.assertRaises(ValueError):
                        with probe.promoted():
                            raise ValueError('acceptance assertion failed')
                self.assertEqual(calls[-1][5:], (1, '--replace', 2))
                self.assertEqual(json.loads((root / 'models.json').read_text()), original)
                promoted = json.loads((root / 'models-v2.json').read_text())
                self.assertEqual(promoted['models'][0]['fields']['closeReason']['as'], {'type': 'optional', 'item': {'type': 'text'}})


if __name__ == '__main__':
    unittest.main()
