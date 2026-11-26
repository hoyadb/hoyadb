"""DEF-0020 derivation validation; synthetic fixtures prove validation only, never performance."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from run import RunnerError, canonical, proof_records
from summary import summarize
from derive_reactive_endpoint import derive

OLD="serialize/facade-call/decoded-return"
NEW="caller-harness-invocation-to-last-callback"
REACTIVE={"N":1000,"Q":0,"mode":"membership","observers":1,"operation":"reactivity","sharing":"equivalent"}
POINT={"N":100,"Q":0,"observers":0,"operation":"point"}
ROOT=Path(__file__).resolve().parents[1]


def oracle(n):
    rows=list(proof_records(n))
    return {"count":len(rows),"sha256":hashlib.sha256(b"".join((canonical(row)+"\n").encode() for row in rows)).hexdigest()}


def sample(phase,index,reactive):
    value=1000+index+(100 if phase=="measured" else 0)
    result={"correct":True,"index":index,"input_bytes":460,"output_bytes":3888,"phase":phase,"status":"ok","value":value}
    if reactive:
        result.update(dispatch_complete_ns=value+50,first_callback_ns=value,last_callback_ns=value,
                      return_to_first_callback_ns=7,return_to_last_callback_ns=7,callback_count=1)
    return result


def write(path,value):
    Path(path).write_text(canonical(value)+"\n",encoding="utf-8")


def experiment(root):
    """Shape of a complete accepted runner output; values are synthetic validation inputs."""
    root=Path(root);root.mkdir()
    source={"sha256":"b"*64,"files":{"benchmarks/run.py":"c"*64}}
    for index,cohort in ((0,POINT),(32,REACTIVE)):
        reports=[]
        for run in range(1,6):
            expected={"artifact_sha256":"a"*64,"class":"rust-facade/memory","endpoint":OLD,"parameters":cohort,
                      "profile":"release","source_sha256":"b"*64,"unit":"ns","workload_sha256":"e"*64}
            label=f"warm-memory-{index:02}-{run}"
            write(root/(label+"-config.json"),{"cohort":cohort,"directory":None,"engine":"memory","identity":expected,"run":run,"run_status":"ok"})
            samples=[sample("warmup",i,index==32) for i in range(5)]+[sample("measured",i,index==32) for i in range(30)]
            report={"format":1,"identity":{**expected,"worker_arch":"x86_64","worker_os":"macos"},"protocol":{"runs":5,"samples":30,"warmups":5},
                    "runs":[{"metadata":{"mode":"warm","proof_complete":True},"run":run,"samples":samples,"status":"ok"}]}
            proof=oracle(cohort["N"])
            write(root/(label+"-raw.json"),{"exit_code":0,"format":1,"launch_to_exit_ns":5,"pid":run,"proof":{"initial":proof,"final":proof},
                                            "report":report,"resource":{"tool":"synthetic"},"status":"ok","stderr":"","stderr_truncated":False})
            reports.append(report)
        write(root/f"warm-memory-{index:02}-summary.json",summarize(reports))
    write(root/"manifest.json",{"accepted_summaries":2,"artifact":{"sha256":"a"*64},"build_exit_code":0,"format":1,
                                "quiet_window_attested_by_caller":True,"run_status":"ok","selection":{"engine":"memory","scope":"warm","warm_cohorts":[0,32]},
                                "source_after":source,"source_before":source,"status":"ok","workload_sha256":"e"*64})
    return root


def hashes(root):
    return {path.relative_to(root).as_posix():hashlib.sha256(path.read_bytes()).hexdigest() for path in sorted(Path(root).rglob("*")) if path.is_file()}


def edit(root,name,change):
    path=Path(root)/name
    value=json.loads(path.read_text(encoding="utf-8"))
    change(value)
    write(path,value)


class ReactiveDerivationTests(unittest.TestCase):
    def test_complete_accepted_reactive_original_changes_only_endpoint_and_keeps_provenance(self):
        with tempfile.TemporaryDirectory() as directory:
            source=experiment(Path(directory)/"original");before=hashes(source);output=Path(directory)/"derived"
            provenance=derive(source,output)
            self.assertEqual(hashes(source),before)
            self.assertEqual(provenance["transform"],"def-0020-reactive-endpoint/1")
            self.assertEqual(provenance["originals"],before)
            self.assertEqual(provenance["derived_groups"],["warm-memory-32"])
            self.assertEqual(provenance["not_derived_groups"],["warm-memory-00"])
            self.assertRegex(provenance["transform_source_sha256"],r"^[a-f0-9]{64}$")
            for run in range(1,6):
                original=json.loads((source/f"warm-memory-32-{run}-raw.json").read_text(encoding="utf-8"))["report"]
                derived=json.loads((output/f"derived-warm-memory-32-{run}-report.json").read_text(encoding="utf-8"))
                self.assertEqual(derived["identity"]["endpoint"],NEW)
                original["identity"]["endpoint"]=NEW
                self.assertEqual(derived,original)
            summary=json.loads((output/"derived-warm-memory-32-summary.json").read_text(encoding="utf-8"))
            accepted=json.loads((source/"warm-memory-32-summary.json").read_text(encoding="utf-8"))
            self.assertEqual(summary["identity"]["endpoint"],NEW)
            accepted["identity"]["endpoint"]=NEW
            self.assertEqual(summary,accepted)
            self.assertEqual(summary["sample_count"],150)
            self.assertFalse(list(output.glob("*warm-memory-00*")))
            self.assertEqual(json.loads((output/"provenance.json").read_text(encoding="utf-8")),provenance)
            self.assertEqual(provenance["derived"],{name:digest for name,digest in hashes(output).items() if name!="provenance.json"})

    def test_failed_interrupted_inconsistent_or_mixed_originals_reject_without_output(self):
        def raw(run,change):
            return lambda root:edit(root,f"warm-memory-32-{run}-raw.json",change)
        def measured(field,value,run=3,index=6):
            def change(item):item["report"]["runs"][0]["samples"][index][field]=value
            return raw(run,change)
        def missing(field):
            def change(item):del item["report"]["runs"][0]["samples"][9][field]
            return raw(2,change)
        def manifest(field,value):
            return lambda root:edit(root,"manifest.json",lambda item:item.__setitem__(field,value))
        cases={
            "failed manifest":manifest("status","failed"),
            "contaminated":manifest("run_status","contaminated"),
            "source drift":manifest("source_after",{"sha256":"f"*64,"files":{}}),
            "missing run":lambda root:(root/"warm-memory-32-5-raw.json").unlink(),
            "missing summary":lambda root:(root/"warm-memory-32-summary.json").unlink(),
            "value differs":measured("value",999999),
            "negative callback":measured("first_callback_ns",-1),
            "noninteger callback":measured("last_callback_ns",1100.5),
            "first after last":measured("first_callback_ns",999999),
            "callback count":measured("callback_count",2),
            "missing callback":missing("return_to_last_callback_ns"),
            "failed sample":measured("status","error"),
            "failed raw":raw(1,lambda item:item.__setitem__("status","failed")),
            "worker exit":raw(1,lambda item:item.__setitem__("exit_code",-15)),
            "proof mismatch":raw(4,lambda item:item["proof"]["final"].__setitem__("sha256","0"*64)),
            "oracle mismatch":raw(4,lambda item:item["proof"].update(initial={"count":1102,"sha256":"0"*64},final={"count":1102,"sha256":"0"*64})),
            "mixed artifact":raw(5,lambda item:item["report"]["identity"].__setitem__("artifact_sha256","9"*64)),
            "already relabelled":raw(1,lambda item:item["report"]["identity"].__setitem__("endpoint",NEW)),
            "incomplete samples":raw(2,lambda item:item["report"]["runs"][0]["samples"].pop()),
            "contaminated run":raw(2,lambda item:item["report"]["runs"][0].__setitem__("status","contaminated")),
            "config drift":lambda root:edit(root,"warm-memory-32-1-config.json",lambda item:item.__setitem__("run_status","contaminated")),
        }
        for name,mutate in cases.items():
            with self.subTest(name),tempfile.TemporaryDirectory() as directory:
                source=experiment(Path(directory)/"original");mutate(source);before=hashes(source);output=Path(directory)/"derived"
                with self.assertRaises(RunnerError):
                    derive(source,output)
                self.assertFalse(output.exists())
                self.assertEqual(hashes(source),before)

    def test_existing_output_and_point_only_experiment_reject(self):
        with tempfile.TemporaryDirectory() as directory:
            source=experiment(Path(directory)/"original");output=Path(directory)/"derived";output.mkdir()
            with self.assertRaises(RunnerError):
                derive(source,output)
            self.assertEqual(list(output.iterdir()),[])
            for path in source.glob("warm-memory-32-*"):path.unlink()
            with self.assertRaises(RunnerError):
                derive(source,Path(directory)/"point-only")
            self.assertFalse((Path(directory)/"point-only").exists())

    def test_interrupted_experiment_rejects_and_stays_byte_identical(self):
        with tempfile.TemporaryDirectory() as directory:
            source=experiment(Path(directory)/"interrupted")
            edit(source,"manifest.json",lambda item:item.__setitem__("status","interrupted"))
            before=hashes(source)
            with self.assertRaises(RunnerError):
                derive(source,Path(directory)/"derived")
            self.assertFalse((Path(directory)/"derived").exists())
            self.assertEqual(hashes(source),before)


if __name__=="__main__":
    unittest.main()
