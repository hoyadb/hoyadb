"""Cost diagnostic parsing/projection; synthetic stage lines prove validation only."""
import unittest
from cost_diagnostic import parse, projection
from run import RunnerError, canonical


def line(stage, wall, index=None, value=None):
    return ("DIAGNOSTIC " + canonical({"index": index, "stage": stage, "timed_value_ns": value, "wall_ns": wall}) + "\n").encode()


VALID = [line("setup", 1000), line("fixture_validation", 50), line("sample", 900, 0, 7), line("sample", 400, 1, 6), line("sample", 500, 2, 5)]


class CostDiagnosticTests(unittest.TestCase):
    def test_exact_stage_sequence_is_parsed_without_inventing_values(self):
        self.assertEqual(parse(VALID, 3), {"setup_wall_ns": 1000, "fixture_validation_wall_ns": 50,
                                           "sample_wall_ns": [900, 400, 500], "timed_reset_ns": [7, 6, 5]})

    def test_missing_extra_reordered_negative_or_noninteger_stages_reject(self):
        cases = [VALID[:-1], VALID + [VALID[-1]], [VALID[1], VALID[0]] + VALID[2:], VALID[:2] + [line("sample", -1, 0, 7)] + VALID[3:],
                 VALID[:2] + [line("sample", 1.5, 0, 7)] + VALID[3:], VALID[:2] + [line("sample", 9, 1, 7)] + VALID[3:],
                 [b"PROOF {}\n"] + VALID, VALID[:2] + [line("sample", 9, 0, None)] + VALID[3:]]
        for index, lines in enumerate(cases):
            with self.subTest(index=index), self.assertRaises(RunnerError):
                parse(lines, 3)

    def test_projection_is_labelled_integer_arithmetic_from_observed_stages_only(self):
        result = projection(parse(VALID, 3))
        # one worker: setup + initial/final validation + first sample + 34 later samples at the observed later mean
        self.assertEqual(result["per_worker_ns"], 1000 + 2 * 50 + 900 + (400 + 500) * 34 // 2)
        self.assertEqual(result["per_cohort_ns"], result["per_worker_ns"] * 5)
        self.assertEqual(result["inputs"], {"warmups": 5, "measured": 30, "runs": 5, "later_samples_observed": 2})
        self.assertIn("not a measurement", result["qualification"])
        self.assertIn("contaminated", result["qualification"])
        with self.assertRaises(RunnerError):
            projection({"setup_wall_ns": 1, "fixture_validation_wall_ns": 1, "sample_wall_ns": [1], "timed_reset_ns": [1]})


if __name__ == "__main__":
    unittest.main()
