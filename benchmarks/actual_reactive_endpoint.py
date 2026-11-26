"""DEF-0020 actual runner regression: real release workers, contaminated, no performance claim.

Run explicitly: python3 -m unittest benchmarks/actual_reactive_endpoint.py
Set HOYADB_ACTUAL_OUTPUT to a new path to retain the raw runner output.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT=Path(__file__).resolve().parents[1]
REACTIVE="caller-harness-invocation-to-last-callback"
DECODED="serialize/facade-call/decoded-return"
CALLBACK_FIELDS=("value","dispatch_complete_ns","first_callback_ns","last_callback_ns",
                 "return_to_first_callback_ns","return_to_last_callback_ns","callback_count")


class ActualReactiveEndpointTests(unittest.TestCase):
    def test_real_runner_names_cohort32_last_callback_and_keeps_point_decoded_return(self):
        with tempfile.TemporaryDirectory(prefix="hoya-def20-") as directory:
            output=Path(os.environ.get("HOYADB_ACTUAL_OUTPUT") or Path(directory)/"output")
            completed=subprocess.run([sys.executable,str(ROOT/"benchmarks/run.py"),"--output",str(output),"--engine","memory",
                                      "--scope","warm","--cohort","0","--cohort","32","--run-status","contaminated"],
                                     cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=3600,check=False)
            manifest=json.loads((output/"manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(completed.returncode,0,manifest.get("failure"))
            self.assertEqual((manifest["status"],manifest["run_status"],manifest["accepted_summaries"]),("contaminated","contaminated",0))
            self.assertEqual(list(output.glob("*-summary.json")),[])
            for run in range(1,6):
                point=json.loads((output/f"warm-memory-00-{run}-raw.json").read_text(encoding="utf-8"))["report"]
                self.assertEqual(point["identity"]["endpoint"],DECODED)
                self.assertTrue(all("last_callback_ns" not in sample for sample in point["runs"][0]["samples"]))
                reactive=json.loads((output/f"warm-memory-32-{run}-raw.json").read_text(encoding="utf-8"))["report"]
                self.assertEqual(reactive["identity"]["parameters"]["operation"],"reactivity")
                self.assertEqual(reactive["runs"][0]["status"],"contaminated")
                samples=reactive["runs"][0]["samples"]
                self.assertEqual([(sample["phase"],sample["index"]) for sample in samples],
                                 [("warmup",index) for index in range(5)]+[("measured",index) for index in range(30)])
                for sample in samples:
                    self.assertTrue(all(type(sample[key]) is int and sample[key]>=0 for key in CALLBACK_FIELDS))
                    self.assertEqual(sample["value"],sample["last_callback_ns"])
                    self.assertLessEqual(sample["first_callback_ns"],sample["last_callback_ns"])
                    self.assertEqual(sample["callback_count"],1)
                self.assertEqual(reactive["identity"]["endpoint"],REACTIVE)


if __name__=="__main__":
    unittest.main()
