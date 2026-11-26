"""Offline controls for exact arithmetic, source-snapshot validation and fail-before-write acceptance."""
import unittest
from unittest.mock import patch
import reconciliation as checks


class ExactTermsTests(unittest.TestCase):
    """Python is an independent exact test oracle, never the product's financial processing runtime."""
    def test_scaled_coefficients_keep_large_values_last_digits_and_negative_zero(self):
        """The final fractional digit survives beyond JavaScript's exact integer range."""
        self.assertEqual(checks.coefficient('9007199254740993.0000000001', 10), 90071992547409930000000001)
        self.assertEqual(checks.coefficient('-0.0000000001', 10), -1)
        self.assertEqual(checks.coefficient('-0.0000000000', 10), 0)
        self.assertEqual(checks.coefficient('0.0000000001', 10) * 10000, 10000)
        for invalid in [True, 0.1, '1e-10', '01.0', '0.00000000001', '9' * 41]:
            with self.subTest(invalid_type=type(invalid).__name__), self.assertRaises(ValueError):
                checks.coefficient(invalid, 10)

    def test_lots_reject_rounded_boolean_and_out_of_range_values(self):
        """Only exact source Int64 transports can contribute to the lot condition."""
        for valid in ['9223372036854775807', 9223372036854775807]:
            self.assertEqual(checks.lot(valid), 9223372036854775807)
        for invalid in [True, 1.0, 2 ** 63, str(-(2 ** 63) - 1), '01', '1e3']:
            with self.subTest(invalid_type=type(invalid).__name__), self.assertRaises(ValueError):
                checks.lot(invalid)


class SnapshotTests(unittest.TestCase):
    """Ledger controls reuse the PoC's read-only query and never invoke lifecycle commands."""
    def invoke(self, entries):
        """Mock only the HTTP boundary and private scope; assert exact offset and read-only endpoint."""
        data = {'package': 'a' * 64, 'parties': {'a': 'A', 'b': 'B'}}
        with patch.object(checks.s, 'state', return_value=data), \
                patch.object(checks.s, 'token', return_value='local-fixture-token'), \
                patch.object(checks.s, 'request', return_value=entries) as request:
            result = checks.s.ledger_snapshot(9007199254740993)
        self.assertEqual(request.call_args.args[1], '/v2/state/active-contracts?limit=100')
        body = request.call_args.kwargs['body']
        self.assertEqual(body['activeAtOffset'], 9007199254740993)
        self.assertEqual(set(body['eventFormat']['filtersByParty']), {'A', 'B'})
        return result

    def entry(self, contract='owned', template=None):
        """Build the verified SDK stream envelope without guessing alternate event shapes."""
        return {'contractEntry': {'JsActiveContract': {'createdEvent': {
            'contractId': contract, 'templateId': template or 'a' * 64 + ':Market:PredictionMarket',
            'createArgument': {'priceTick': '0.0000000001', 'quantityLot': '9223372036854775807'}}}}}

    def test_snapshot_is_exact_sorted_and_package_scoped(self):
        """Membership is checked independently from indexer totals and projection rows."""
        rows = self.invoke([self.entry('z'), self.entry('a')])
        self.assertEqual([row['contractId'] for row in rows], ['a', 'z'])

    def test_truncation_unknown_shapes_duplicates_and_package_changes_are_rejected(self):
        """Missing or ambiguous source evidence must not become a successful smaller set."""
        for entries in [[self.entry(template='unexpected')], [self.entry(), self.entry()],
                        [{'contractEntry': {'JsIncompleteAssigned': {}}}], [{}] * 100]:
            with self.subTest(case_length=len(entries)), self.assertRaises(ValueError):
                self.invoke(entries)


class IndependentInventoryTests(unittest.TestCase):
    def test_mismatched_source_inventory_and_values_fail_before_destination_writes(self):
        """A wrong independent expectation cannot be repaired by installing a convenient model."""
        source = [{'contractId': 'a', 'payload': {'priceTick': '0.0000000001', 'quantityLot': '3'}}]
        for ledger in [[{'contractId': 'different', 'payload': source[0]['payload']}],
                       [{'contractId': 'a', 'payload': {'priceTick': '0.0000000002', 'quantityLot': '3'}}]]:
            with patch.object(checks.s, 'ledger_snapshot', return_value=ledger), \
                    patch.object(checks, 'write_private') as write:
                cli = unittest.mock.Mock()
                with self.assertRaises(ValueError):
                    checks.configure({}, None, 'owned', cli, source, 10)
                cli.assert_not_called()
                write.assert_not_called()
