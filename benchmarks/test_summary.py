"""Synthetic statistics fixtures only; never performance evidence."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from summary import InvalidSamples, read, summarize


def document(values=range(1, 31)):
    values = list(values)
    return {
        "format": 1,
        "identity": {"class": "synthetic-unit-test", "profile": "release", "unit": "ns",
                     "artifact_sha256": "a" * 64, "workload_sha256": "b" * 64,
                     "endpoint": "synthetic", "parameters": {"N": 100}},
        "protocol": {"runs": 5, "warmups": 5, "samples": len(values)},
        "runs": [{"run": run, "status": "ok", "samples": [
            {"index": index, "phase": "warmup", "value": 99999, "status": "ok", "correct": True}
            for index in range(5)] + [
            {"index": index, "phase": "measured", "value": value, "status": "ok", "correct": True}
            for index, value in enumerate(values)]} for run in range(1, 6)],
    }


class SummaryTests(unittest.TestCase):
    def test_even_median_and_nearest_rank_p95_are_exact(self):
        result = summarize([document()])
        self.assertEqual(result["sample_count"], 150)
        self.assertEqual(result["runs"][0], {"run": 1, "count": 30, "median": "15.5", "p95": 29, "min": 1, "max": 30})
        self.assertEqual(result["envelope"], {"median_run_median": "15.5", "min_run_p95": 29, "max_run_p95": 29})

    def test_odd_single_repeated_and_unsorted_values(self):
        for values, median, p95 in [([100, 1, 2], "2", 100), ([8], "8", 8), ([4, 4, 4], "4", 4)]:
            with self.subTest(values=values):
                result = summarize([document(values)])["runs"][0]
                self.assertEqual((result["median"], result["p95"]), (median, p95))

    def test_u64_median_does_not_round_through_float(self):
        values = [(1 << 64) - 2, (1 << 64) - 1]
        self.assertEqual(summarize([document(values)])["runs"][0]["median"], "18446744073709551614.5")

    def test_empty_missing_malformed_or_nonfinite_samples_reject(self):
        bad = [document([]), {}, {"format": 1}, None]
        for value in [-1, True, 1.5, float("nan"), float("inf"), "10", 1 << 64]:
            bad.append(document([value]))
        for item in bad:
            with self.subTest(item=repr(item)[:80]), self.assertRaises(InvalidSamples):
                summarize([item])
        with self.assertRaises(InvalidSamples):
            summarize([])

    def test_mixed_identity_units_profiles_engines_and_artifacts_reject(self):
        for field, value in [("unit", "ms"), ("profile", "debug"), ("class", "other-engine"),
                             ("artifact_sha256", "c" * 64), ("workload_sha256", "d" * 64)]:
            other = document()
            other["identity"][field] = value
            with self.subTest(field=field), self.assertRaises(InvalidSamples):
                summarize([document(), other])

    def test_sample_level_unit_and_profile_overrides_cannot_hide_mixed_inputs(self):
        for key,value in [("unit","ms"),("profile","debug")]:
            item=document()
            item["runs"][0]["samples"][5][key]=value
            with self.subTest(key=key),self.assertRaises(InvalidSamples):
                summarize([item])

    def test_failed_incorrect_contaminated_missing_or_duplicate_samples_reject(self):
        mutations = [lambda d: d["runs"][0].update(status="contaminated"),
                     lambda d: d["runs"][0]["samples"][5].update(status="timeout"),
                     lambda d: d["runs"][0]["samples"][5].update(correct=False),
                     lambda d: d["runs"].pop(),
                     lambda d: d["runs"][0]["samples"].pop(),
                     lambda d: d["runs"][0]["samples"][6].update(index=0),
                     lambda d: d["runs"][1].update(run=1)]
        for mutate in mutations:
            item = document()
            mutate(item)
            before = copy.deepcopy(item)
            with self.assertRaises(InvalidSamples):
                summarize([item])
            self.assertEqual(item, before, "validation must not change raw data")

    def test_reader_rejects_nonstandard_numbers_duplicate_keys_and_malformed_json(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "samples.json"
            for raw in ["{", '{"v":NaN}', '{"v":Infinity}', '{"v":1,"v":2}']:
                path.write_text(raw)
                with self.subTest(raw=raw), self.assertRaises(InvalidSamples):
                    read(path)
            path.write_text(json.dumps(document()))
            self.assertEqual(read(path), document())

    def test_independent_runs_remain_separate_and_warmups_do_not_change_tail(self):
        item = document()
        for sample in item["runs"][4]["samples"]:
            if sample["phase"] == "measured":
                sample["value"] *= 10
        result = summarize([item])
        self.assertEqual([r["p95"] for r in result["runs"]], [29, 29, 29, 29, 290])
        self.assertEqual(result["envelope"]["max_run_p95"], 290)
        self.assertEqual(result["envelope"]["median_run_median"], "15.5")


if __name__ == "__main__":
    unittest.main()
