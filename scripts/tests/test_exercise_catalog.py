"""M0 catalog controls use synthetic responses; no Docker, Ledger or SQL is contacted."""
import importlib
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
probe = importlib.import_module('devtools.checks.exercises')


def catalog():
    """The function body sentinel must remain private even in successful public summaries."""
    return {'schema_version': '041', 'read_only': True, 'restricted_reader': True,
            'functions': [{'name': 'exercises', 'arguments': 'qname text, from_offset bigint, to_offset bigint',
                           'result': 'SETOF exercise', 'definition': 'private-function-body-sentinel'}],
            'attributes': [{'type': 'exercise', 'name': 'witnesses', 'sql_type': 'text[]'},
                           {'type': 'event_id', 'name': 'node_id', 'sql_type': 'integer'}],
            'columns': []}


class ExerciseCatalogTests(unittest.TestCase):
    """Catalog presence is explicitly weaker evidence than an authorized event/lineage observation."""

    def test_read_only_reader_and_public_summary_never_claim_live_coverage(self):
        data = catalog()
        with patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, json.dumps(data), '')) as run:
            result = probe.inspect_catalog()
        self.assertIn('cantongrid_pqs_reader', run.call_args.args[0])
        self.assertIn('READ ONLY', run.call_args.kwargs['input'])
        self.assertNotIn('FROM exercises(', run.call_args.kwargs['input'])
        summary = probe.summarize(result)
        self.assertNotIn('private-function-body-sentinel', json.dumps(summary))
        self.assertFalse(summary['live_authorization_verified'])
        self.assertFalse(summary['exercise_visibility_verified'])
        self.assertFalse(summary['lineage_enabled'])
        self.assertEqual(summary['event_id_attributes'][0]['name'], 'node_id')

    def test_incompatible_privileged_malformed_and_truncated_profiles_fail_closed(self):
        cases = []
        for field, value in [('schema_version', '042'), ('read_only', False), ('restricted_reader', False),
                             ('functions', [{}] * 32), ('attributes', None)]:
            data = catalog()
            data[field] = value
            cases.append(json.dumps(data))
        data = catalog()
        data['functions'][0]['definition'] = None
        cases.extend([json.dumps(data), 'not json', 'x' * (probe.MAX_BYTES + 1)])
        for body in cases:
            with self.subTest(size=len(body)), patch.object(probe.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, body, '')):
                with self.assertRaises(ValueError):
                    probe.inspect_catalog()

    def test_failed_reader_does_not_fall_back_to_owner(self):
        with patch.object(probe.subprocess, 'run', side_effect=subprocess.CalledProcessError(1, [])) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                probe.inspect_catalog()
        self.assertEqual(run.call_count, 1)

    def test_fixture_build_uses_isolated_space_free_copy_and_sdk_launcher(self):
        with patch.object(probe.shutil, 'which', return_value='/sdk/dpm'), \
                patch.object(probe.subprocess, 'run') as run:
            probe.test_fixture('/sdk/dpm')
        self.assertEqual([call.args[0] for call in run.call_args_list],
                         [['/sdk/dpm', 'build'], ['/sdk/dpm', 'test']])
        for call in run.call_args_list:
            self.assertNotIn(' ', str(call.kwargs['cwd']))
            self.assertNotIn(str(probe.ROOT), str(call.kwargs['cwd']))
            self.assertFalse(call.kwargs['cwd'].exists())
