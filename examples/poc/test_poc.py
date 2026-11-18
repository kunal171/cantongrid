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


if __name__ == '__main__':
    unittest.main()
