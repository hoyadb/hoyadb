"""A08 actual native admission integration: real release workers, contaminated, no performance claim.

Run explicitly: python3 -m unittest benchmarks/actual_admission.py
Set HOYADB_ACTUAL_OUTPUT to a new path to retain the raw runner output.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest

ROOT=Path(__file__).resolve().parents[1]


def children(pid):
    found=subprocess.run(["pgrep","-P",str(pid)],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,check=False)
    return found.stdout.split()


class ActualAdmissionTests(unittest.TestCase):
    def test_real_runner_holds_after_each_reaped_worker_until_explicit_exact_admission(self):
        with tempfile.TemporaryDirectory(prefix="hoya-admission-") as directory:
            output=Path(os.environ.get("HOYADB_ACTUAL_OUTPUT") or Path(directory)/"output")
            process=subprocess.Popen([sys.executable,str(ROOT/"benchmarks/run.py"),"--output",str(output),"--engine","memory",
                                      "--scope","warm","--cohort","0","--run-status","contaminated","--admission","stdin"],
                                     cwd=ROOT,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True)
            holds=[];admitted=[]
            try:
                for line in process.stdout:
                    if line.startswith("BETWEEN_WORKERS "):
                        held=json.loads(line[len("BETWEEN_WORKERS "):])
                        holds.append(held)
                        # A wrongly automatic next launch would appear during this observed hold.
                        time.sleep(0.5)
                        self.assertEqual(children(process.pid),[])
                        self.assertEqual(len(list(output.glob("*-raw.json"))),len(holds))
                        if len(holds)==1:
                            process.stdin.write("DECLARE "+json.dumps({"work":"actual admission integration declaration"},separators=(",",":"))+"\n")
                            process.stdin.flush()
                    elif line.startswith("AWAIT_ADMISSION "):
                        self.assertEqual(len(admitted),len(holds))
                        self.assertEqual(children(process.pid),[])
                        coordinates=line[len("AWAIT_ADMISSION "):].rstrip("\n")
                        admitted.append(json.loads(coordinates))
                        process.stdin.write("ADMIT "+coordinates+"\n")
                        process.stdin.flush()
            finally:
                process.stdin.close()
                returncode=process.wait(timeout=600)
                process.stdout.close()
            manifest=json.loads((output/"manifest.json").read_text(encoding="utf-8"))
            self.assertEqual(returncode,0,manifest.get("failure"))
            self.assertEqual((manifest["status"],manifest["accepted_summaries"]),("contaminated",0))
            self.assertEqual([item["label"] for item in admitted],[f"warm-memory-00-{run}" for run in range(1,6)])
            self.assertEqual([item["label"] for item in holds],[f"warm-memory-00-{run}" for run in range(1,6)])
            events=manifest["admission"]["events"]
            self.assertEqual([event["event"] for event in events],["admit","between_workers","declare"]+["admit","between_workers"]*4)
            for run,held in enumerate(holds,1):
                raw=output/f"warm-memory-00-{run}-raw.json"
                self.assertEqual(held["raw_sha256"],__import__("hashlib").sha256(raw.read_bytes()).hexdigest())
                self.assertEqual(held["pid"],json.loads(raw.read_text(encoding="utf-8"))["pid"])
            admits=[event for event in events if event["event"]=="admit"]
            for previous,following in zip(holds,admits[1:]):
                self.assertEqual(following["hold"],previous)
                self.assertLess(previous["monotonic_ns"],following["monotonic_ns"])


if __name__=="__main__":
    unittest.main()
