"""Credential-free checks for the PoC's evidence and local state transitions."""
import unittest
from unittest.mock import patch
import lifecycle


class RepeatRunTests(unittest.TestCase):
    def test_resolved_market_is_archived_before_next_cycle(self):
        saved = {'source': 'poc-test', 'stage': 'Resolved', 'cycle': 1,
                 'parties': {'a': 'A', 'b': 'B'}, 'contracts': {'shared': 'old'}, 'expectedEvents': []}
        calls = []

        def submit(command, marker, expected_creates=1):
            calls.append((command, marker, expected_creates))
            return 'new' if expected_creates else None

        with patch.object(lifecycle, 'state', side_effect=lambda: saved.copy()), \
                patch.object(lifecycle, 'save', side_effect=lambda data: saved.update(data)), \
                patch.object(lifecycle, 'submit', side_effect=submit):
            lifecycle.create()
        self.assertEqual(calls[0][0]['ExerciseCommand']['choice'], 'Archive')
        self.assertEqual(calls[0][2], 0)
        self.assertIn('CreateCommand', calls[1][0])
        self.assertEqual(saved['stage'], 'Draft')
        self.assertEqual(saved['contracts']['shared'], 'new')

    def test_open_market_cannot_be_silently_replaced(self):
        with patch.object(lifecycle, 'state', return_value={'stage': 'Open'}), \
                patch.object(lifecycle, 'submit') as submit:
            with self.assertRaises(ValueError):
                lifecycle.create()
            submit.assert_not_called()


class QuerySafetyTests(unittest.TestCase):
    def test_revision_is_restored_when_acceptance_assertion_fails(self):
        import json
        import tempfile
        from pathlib import Path
        import queries
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'models.json').write_text(json.dumps({'version': 1, 'models': [{'revision': 1}]}))
            calls = []
            with patch.object(queries.s, 'DIRECTORY', root), \
                    patch.object(queries.s, 'state', return_value={'namespace': 'owned'}), \
                    patch.object(queries.s, 'cli', side_effect=lambda *args: calls.append(args)):
                with self.assertRaisesRegex(ValueError, 'acceptance failed'):
                    with queries.promoted():
                        raise ValueError('acceptance failed')
            self.assertEqual(calls[-1], ('models', 'bind', root / 'runtime.json', 'owned', 'markets', 1, '--replace', 2))
            self.assertEqual(json.loads((root / 'models.json').read_text())['models'][0]['revision'], 1)

    def test_revision_restore_also_runs_after_ambiguous_promotion_failure(self):
        import json
        import tempfile
        from pathlib import Path
        import queries
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'models.json').write_text(json.dumps({'version': 1, 'models': [{'revision': 1}]}))
            calls = []

            def cli(*args):
                calls.append(args)
                if args[:2] == ('models', 'bind') and args[5] == 2:
                    raise ValueError('promotion acknowledgement lost')

            with patch.object(queries.s, 'DIRECTORY', root), \
                    patch.object(queries.s, 'state', return_value={'namespace': 'owned'}), \
                    patch.object(queries.s, 'cli', side_effect=cli):
                with self.assertRaisesRegex(ValueError, 'acknowledgement lost'):
                    with queries.promoted():
                        self.fail('failed promotion must not enter acceptance body')
            self.assertEqual(calls[-1][5:], (1, '--replace', 2))


class RebuildOptInTests(unittest.TestCase):
    def test_rebuild_requires_explicit_ledger_write_opt_in(self):
        import demo
        with patch('sys.argv', ['demo.py', 'rebuild']), patch('sys.stderr'):
            with self.assertRaises(SystemExit) as stopped:
                demo.main()
        self.assertEqual(stopped.exception.code, 2)


if __name__ == '__main__':
    unittest.main()
