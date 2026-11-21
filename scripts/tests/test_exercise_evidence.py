"""Synthetic M0 receipts test lost evidence and privacy boundaries without a live Ledger."""
import copy
import importlib
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
evidence = importlib.import_module('devtools.checks.exercise_evidence')


def receipts():
    """Use an offset above JavaScript's exact range; source IDs and quantities stay exact."""
    offset = 9007199254740993
    exercise = {'offset': offset, 'nodeId': 0, 'contractId': 'parent', 'templateId': 'package:M:T',
        'witnessParties': ['operator'], 'choice': 'Create', 'consuming': True,
        'choiceArgument': {'quantity': '9223372036854775807'}, 'exerciseResult': 'child',
        'actingParties': ['operator'], 'lastDescendantNodeId': 1}
    create = {'offset': offset, 'nodeId': 1, 'contractId': 'child', 'templateId': 'package:M:C',
        'witnessParties': ['operator', 'a'], 'createArgument': {'declaredParent': 'parent'},
        'signatories': ['operator'], 'observers': ['a']}
    ledger = {'control': {'transaction': {'events': [{'ExercisedEvent': exercise}, {'CreatedEvent': create}]}}}
    pqs = {'exercises': [{'exercise_event_id': {'offset': offset, 'node_id': 0},
        'contract_id': 'parent', 'template_fqn': 'package-name:M:T', 'package_id': 'package',
        'package_name': 'package-name', 'witnesses': ['operator'],
        'choice': 'Create', 'consuming': True, 'argument': exercise['choiceArgument'],
        'result': 'child', 'controllers': ['operator'], 'last_descendant_node_id': 1}],
        'creates': [{'create_event_id': {'offset': offset, 'node_id': 1},
        'contract_id': 'child', 'template_fqn': 'package-name:M:C', 'package_id': 'package',
        'package_name': 'package-name', 'witnesses': ['a', 'operator'],
        'payload': create['createArgument'], 'signatories': ['operator'], 'observers': ['a']}],
        'archives': [{'archive_event_id': {'offset': offset, 'node_id': 0}, 'contract_id': 'parent',
                      'template_fqn': 'package-name:M:T', 'package_id': 'package',
                      'package_name': 'package-name', 'witnesses': ['operator']}]}
    return ledger, pqs


class ExerciseEvidenceTests(unittest.TestCase):
    """Receipt agreement cannot be silently promoted into direct-parent or complete-history proof."""

    def test_exact_receipts_and_partial_view_do_not_infer_relationships(self):
        ledger, pqs = receipts()
        result = evidence.compare_view(ledger, pqs)
        self.assertTrue(all(row['identities_match'] and row['values_match'] for row in result.values()))
        ledger['control']['transaction']['events'].pop(0)
        pqs['exercises'] = []
        pqs['archives'] = []
        result = evidence.compare_view(ledger, pqs)
        self.assertEqual(result['exercises']['ledger'], 0)
        self.assertNotIn('parent', result)

    def test_missing_and_changed_witness_payload_or_bounds_are_detected(self):
        ledger, pqs = receipts()
        for field, value in [('witnesses', ['a']), ('last_descendant_node_id', -1),
                             ('argument', {'quantity': 9223372036854775807})]:
            changed = copy.deepcopy(pqs)
            changed['exercises'][0][field] = value
            result = evidence.compare_view(ledger, changed)['exercises']
            self.assertTrue(result['identities_match'])
            self.assertFalse(result['values_match'])
        pqs['creates'] = []
        self.assertFalse(evidence.compare_view(ledger, pqs)['creates']['identities_match'])

    def test_duplicates_and_redaction_are_rejected(self):
        ledger, pqs = receipts()
        pqs['creates'].append(pqs['creates'][0])
        with self.assertRaises(ValueError):
            evidence.compare_view(ledger, pqs)
        ledger, pqs = receipts()
        pqs['exercises'][0]['redaction_id'] = 'redacted'
        with self.assertRaises(ValueError):
            evidence.compare_view(ledger, pqs)
