"""Application fixture tests preserve integer/float kinds and tuple correlations."""
import unittest
from consumer_server import outcomes


class Outcomes(unittest.TestCase):
    def test_command_business_echo_keeps_arbitrary_precision(self):
        args = {'min': -9223372036854775808, 'max': 9223372036854775807, 'float': 42.0}
        self.assertEqual(outcomes([{'idempotencyKey': 'k', 'command': 'arbitrary-v1', 'args': args}]), [{'idempotencyKey': 'k', 'status': 'applied', 'result': args}])
        self.assertIsInstance(outcomes([{'idempotencyKey': 'k', 'command': 'arbitrary-v1', 'args': args}])[0]['result']['float'], float)

    def test_mixed_reversed_results_keep_each_entity_write_id(self):
        values = [{'idempotencyKey': 'k', 'command': 'one-v1', 'args': {}}, *[{'table': 'caller_table', 'recordId': 'same', 'writeId': key, 'op': 'create' if key == 'a' else 'update', 'changes': {}} for key in ['a', 'b']]]
        result = outcomes(values)
        self.assertEqual([value.get('writeId', value.get('idempotencyKey')) for value in result], ['b', 'a', 'k'])
        self.assertEqual(result[1]['result'], {'id': 'same', 'created': True, 'echo': {}})

    def test_business_rejection_is_an_explicit_outcome(self):
        self.assertEqual(outcomes([{'idempotencyKey': 'k', 'command': 'one-v1', 'args': {}}], 'rejected'), [{'idempotencyKey': 'k', 'status': 'rejected', 'reason': 'application fixture rejection'}])


if __name__ == '__main__':
    unittest.main()
