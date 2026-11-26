"""API-0008/API-0009: command/entity and paged-pull reference assertions."""
import json
import unittest
from command_http import response, pull_response

class ReferenceTransport(unittest.TestCase):
    def test_phase_five_pull_pages_are_bounded_resumable_and_exact(self):
        code, raw = pull_response()
        self.assertEqual(code, 200)
        first = json.loads(raw)
        self.assertFalse(first["complete"])
        self.assertEqual(first["changes"][0]["changes"]["country"]["value"], "first 東京\u0000🌿")
        code, raw = pull_response(first["checkpoint"])
        second = json.loads(raw)
        self.assertEqual(code, 200)
        self.assertTrue(second["complete"])
        self.assertNotEqual(second["checkpoint"], first["checkpoint"])
        self.assertEqual(json.loads(pull_response(second["checkpoint"])[1])["changes"], [])
        self.assertEqual(pull_response("foreign")[0], 400)
        code, raw = pull_response(None, "large")
        self.assertEqual(code, 200)
        self.assertGreater(len(raw), 262144)
        self.assertLessEqual(len(raw), 1048576)

    def test_phase_five_mixed_entities_keep_exact_correlation_without_idempotency_key(self):
        commands = json.loads(self.body(1))
        entity = {"table": "customers", "op": "create", "recordId": "entity:" + "a" * 32 + ":0000000000000001",
                  "changes": {"country": "東京\u0000🌿"}, "removedFields": [],
                  "clientTimestamp": "1970-01-01T00:00:01.000Z", "schemaVersion": 1,
                  "context": {"identity": None, "capturedAt": "1970-01-01T00:00:01.000Z"},
                  "writeId": "synthetic-write-1"}
        commands.extend([entity, {**entity, "writeId": "synthetic-write-2"}])
        code, raw = response(json.dumps(commands, ensure_ascii=False).encode())
        self.assertEqual(code, 200)
        results = json.loads(raw)
        self.assertEqual([entry.get("writeId") for entry in results[:2]], ["synthetic-write-2", "synthetic-write-1"])
        self.assertEqual(results[0]["recordId"], entity["recordId"])
        self.assertEqual(results[0]["result"]["id"], entity["recordId"])
        self.assertIs(results[0]["result"].get("created"), True)
        self.assertNotIn("idempotencyKey", results[0])

    def body(self, count=3):
        return json.dumps([{
            "command": "sample-v1", "idempotencyKey": "key-" + str(i),
            "schemaVersion": 1, "context": {"identity": None, "capturedAt": "synthetic"},
            "args": {"max": 9223372036854775807, "min": -9223372036854775808,
                     "integral": 42.0, "fraction": 1.25, "unicode": "🌿\u0000"}
        } for i in range(count)], ensure_ascii=False).encode()

    def result(self, mode="applied", count=3):
        code, raw = response(self.body(count), mode)
        self.assertEqual(code, 200)
        return json.loads(raw)

    def test_one_many_reversed_exact_numbers_and_arbitrary_result(self):
        self.assertEqual(len(self.result(count=1)), 1)
        result = self.result()
        self.assertEqual([r["idempotencyKey"] for r in result], ["key-2", "key-1", "key-0"])
        echo = result[0]["result"]["echo"]
        self.assertEqual(echo["max"], 9223372036854775807)
        self.assertEqual(echo["min"], -9223372036854775808)
        self.assertIs(type(echo["integral"]), float)
        self.assertEqual(echo["unicode"], "🌿\u0000")

    def test_independent_outcomes_and_partial_duplicate_conflict(self):
        self.assertEqual({r["status"] for r in self.result("mixed")}, {"applied", "corrected", "rejected"})
        self.assertEqual(len(self.result("partial")), 1)
        duplicate = self.result("duplicate")
        self.assertEqual(duplicate[:3], duplicate[3:])
        conflict = self.result("conflict")
        self.assertEqual(conflict[0]["idempotencyKey"], conflict[-1]["idempotencyKey"])
        self.assertNotEqual(conflict[0], conflict[-1])
        self.assertEqual(response(self.body(), "unavailable")[0], 503)

    def test_rejects_empty_entity_diff_malformed_oversize_and_unknown_mode(self):
        for body in [b'[]', b'{}', b'[', b'[{"table":"orders","op":"insert"}]', b' ' * 1048577,
                     b'[{"command":"a","command":"b"}]']:
            self.assertEqual(response(body)[0], 400)
        self.assertEqual(response(self.body(), "unknown")[0], 400)

if __name__ == "__main__":
    unittest.main()
