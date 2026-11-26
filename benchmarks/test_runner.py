"""Independent fixture/stream/lock checks; synthetic data, not benchmark results."""
import copy
import hashlib
import json
import unittest
import tempfile
import sys
from pathlib import Path
from run import Admission, ProofCapture, arguments, RunnerError, audit_locks, proof_records, execute_worker, source_snapshot, checked_report


def canonical(value):
    return json.dumps(value,ensure_ascii=False,sort_keys=True,separators=(",",":"))


def transcript(n=100):
    rows=list(proof_records(n))
    lines=[]
    for phase in ["initial","final"]:
        lines.append(f"PROOF_BEGIN {phase}\n".encode())
        lines += [("PROOF "+canonical(row)+"\n").encode() for row in rows]
        lines.append(f"PROOF_END {phase}\n".encode())
    report={"format":1,"identity":{"class":"synthetic-test"},"runs":[]}
    lines.append(("REPORT "+canonical(report)+"\n").encode())
    return rows,lines,report


class RunnerTests(unittest.TestCase):
    def test_literal_A_oracle_has_exact_kinds_fields_order_and_counts(self):
        rows=list(proof_records(100))
        self.assertEqual(len(rows),112)
        self.assertEqual(rows[0],{"table":"articles","row":{"id":"article:000000","fields":{
            "title":{"kind":"string","value":"東京🌿-0"},
            "score":{"kind":"integer","value":"0"},"ratio":{"kind":"float","value":"0.25"},
            "active":{"kind":"boolean","value":True},"at":{"kind":"timestamp","value":"1700000000000"},
            "author_id":{"kind":"reference","value":"author:000000"}}}})
        self.assertEqual(rows[1]["row"]["fields"]["note"],{"kind":"string","value":"note-1"})
        self.assertEqual(rows[100],{"table":"authors","row":{"id":"author:000000","fields":{"name":{"kind":"string","value":"author-000000"}}}})
        self.assertEqual(rows[-2:], [{"table":"tags","row":None},{"table":"article_tags","row":None}])
        with self.assertRaises(RunnerError):
            list(proof_records(99))

    def test_stream_hashes_exact_real_record_bytes_and_keeps_only_proof_counts(self):
        rows,lines,report=transcript()
        capture=ProofCapture(100)
        for line in lines:
            capture.feed(line)
        result=capture.finish()
        expected=hashlib.sha256(b"".join((canonical(row)+"\n").encode() for row in rows)).hexdigest()
        self.assertEqual(result["report"],report)
        self.assertEqual(result["proof"],{"initial":{"count":112,"sha256":expected},"final":{"count":112,"sha256":expected}})
        self.assertNotIn("rows",result)

    def test_wrong_type_value_order_missing_extra_or_unknown_output_rejects(self):
        _,lines,_=transcript()
        variants=[]
        bad=lines.copy();record=json.loads(bad[1][6:]);record["row"]["fields"]["score"]={"kind":"float","value":"0"};bad[1]=("PROOF "+canonical(record)+"\n").encode();variants.append(bad)
        bad=lines.copy();bad[1],bad[2]=bad[2],bad[1];variants.append(bad)
        variants.append(lines[:-1])
        variants.append([b"LOG unexpected output\n"]+lines)
        bad=lines.copy();bad.insert(113,lines[1]);variants.append(bad)
        bad=lines.copy();bad.pop(2);variants.append(bad)
        for item in variants:
            with self.subTest(length=len(item)),self.assertRaises(RunnerError):
                capture=ProofCapture(100)
                for line in item:
                    capture.feed(line)
                capture.finish()

    def test_duplicate_keys_noncanonical_data_nonfinite_and_oversize_reject(self):
        for line in [b'PROOF {"table":"articles","table":"authors","row":null}\n',
                     b'PROOF {"table":"articles","row":NaN}\n',
                     b'PROOF { "table":"articles","row":null}\n',
                     b'PROOF '+b'x'*(1048576+1)+b'\n']:
            with self.subTest(size=len(line)),self.assertRaises(RunnerError):
                capture=ProofCapture(100);capture.feed(b"PROOF_BEGIN initial\n");capture.feed(line)

    def test_lock_graph_is_exact_subset_and_path_package_addition_only(self):
        base={"package":[{"name":"old","version":"1.2.3","source":"registry+x","checksum":"abc"},
                         {"name":"hoyadb-core","version":"0.0.0"}]}
        standalone=copy.deepcopy(base)
        standalone["package"].append({"name":"hoyadb-benchmark","version":"0.0.0"})
        self.assertEqual(audit_locks(base,standalone),1)
        for key,value in [("version","1.2.4"),("checksum","def"),("source","registry+y")]:
            wrong=copy.deepcopy(standalone);wrong["package"][0][key]=value
            with self.subTest(key=key),self.assertRaises(RunnerError):
                audit_locks(base,wrong)
        wrong=copy.deepcopy(standalone);wrong["package"].append({"name":"new-dep","version":"1"})
        with self.assertRaises(RunnerError):
            audit_locks(base,wrong)


    def test_worker_capture_is_linewise_retains_raw_and_never_overwrites(self):
        _,lines,report=transcript()
        code="import sys; sys.stdout.buffer.write("+repr(b"".join(lines))+"); sys.stderr.write('synthetic note\\n')"
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/"raw.json"
            result=execute_worker([sys.executable,"-c",code],100,path)
            self.assertEqual(result["status"],"ok")
            self.assertEqual(result["report"],report)
            self.assertEqual(result["proof"]["final"]["count"],112)
            self.assertEqual(result["exit_code"],0)
            self.assertGreater(result["pid"],0)
            self.assertGreater(result["launch_to_exit_ns"],0)
            self.assertEqual(json.loads(path.read_text()),result)
            self.assertNotIn("東京",path.read_text())
            with self.assertRaises(FileExistsError):
                execute_worker([sys.executable,"-c",code],100,path)

    def test_failed_or_invalid_worker_is_preserved_without_fake_measurements(self):
        _,lines,_=transcript()
        cases=[("import sys;sys.stderr.write('setup failure');sys.exit(7)",7),
               ("import sys;sys.stdout.buffer.write(b'WRONG output\\n');sys.stdout.flush()",None),
               ("import sys;sys.stdout.buffer.write("+repr(b"".join(lines[:-1]))+")",0)]
        with tempfile.TemporaryDirectory() as directory:
            for index,(code,exit_code) in enumerate(cases):
                path=Path(directory)/f"raw-{index}.json"
                result=execute_worker([sys.executable,"-c",code],100,path)
                self.assertEqual(result["status"],"failed")
                self.assertIn("failure",result)
                self.assertNotIn("samples",result)
                if exit_code is not None:
                    self.assertEqual(result["exit_code"],exit_code)
                self.assertEqual(json.loads(path.read_text()),result)

    def test_source_identity_detects_real_edits_and_excludes_build_products(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root/"benchmarks/native/src").mkdir(parents=True)
            (root/"benchmarks/native/target").mkdir()
            source=root/"benchmarks/native/src/main.rs"
            source.write_text("source1")
            (root/"benchmarks/native/target/output.rs").write_text("generated")
            first=source_snapshot(root,["benchmarks"])
            self.assertEqual(list(first["files"]),["benchmarks/native/src/main.rs"])
            (root/"benchmarks/native/target/output.rs").write_text("changed output")
            self.assertEqual(source_snapshot(root,["benchmarks"]),first)
            source.write_text("source2")
            self.assertNotEqual(source_snapshot(root,["benchmarks"])["sha256"],first["sha256"])

    def test_empty_or_missing_worker_package_cannot_pass_lock_audit(self):
        base={"package":[{"name":"old","version":"1","source":"x","checksum":"abc"}]}
        for standalone in ({"package":[]},base):
            with self.assertRaises(RunnerError):
                audit_locks(base,standalone)


    def test_cold_ready_is_once_before_proof_and_zero_dataset_is_independent(self):
        rows,lines,report=transcript(0)
        self.assertEqual(rows,[{"table":"tags","row":None},{"table":"article_tags","row":None}])
        capture=ProofCapture(0)
        capture.feed(b"READY\n")
        for line in lines:
            capture.feed(line)
        self.assertTrue(capture.finish()["ready"])
        for bad in ([b"READY\n",b"READY\n"],[lines[0],b"READY\n"],lines+[b"READY\n"]):
            with self.assertRaises(RunnerError):
                capture=ProofCapture(0)
                for line in bad:
                    capture.feed(line)


    def test_acceptance_requires_frozen_artifact_source_and_real_proof(self):
        identity={"artifact_sha256":"a"*64,"profile":"release"}
        source={"sha256":"b"*64,"files":{"main.rs":"c"*64}}
        raw={"status":"ok","exit_code":0,"report":{"identity":identity.copy(),"runs":[]},
             "proof":{"initial":{"count":112,"sha256":"d"*64},"final":{"count":112,"sha256":"d"*64}}}
        self.assertEqual(checked_report(raw,identity,source,source),raw["report"])
        wrong=[]
        for field,value in [("status","failed"),("exit_code",1),("proof",{})]:
            item=copy.deepcopy(raw);item[field]=value;wrong.append(item)
        item=copy.deepcopy(raw);item["report"]["identity"]["artifact_sha256"]="e"*64;wrong.append(item)
        item=copy.deepcopy(raw);item["proof"]["final"]["sha256"]="e"*64;wrong.append(item)
        for item in wrong:
            with self.assertRaises(RunnerError):
                checked_report(item,identity,source,source)
        with self.assertRaises(RunnerError):
            checked_report(raw,identity,source,{"sha256":"e"*64})


    def test_rss_handshake_is_untimed_owned_and_requires_all_61_checkpoints(self):
        _,lines,report=transcript()
        cut=lines.index(b"PROOF_END initial\n")+1
        initial=lines[:cut];final=lines[cut:-1]
        def checkpoint(phase,cycle):
            return ("RSS "+canonical({"phase":phase,"cycle":cycle})+"\n").encode()
        sequence=[checkpoint("baseline",None)]
        for cycle in range(30):
            sequence.append(checkpoint("after_open",cycle))
            if cycle==0: sequence+=initial
            if cycle==29: sequence+=final
            sequence.append(checkpoint("after_close",cycle))
        sequence.append(lines[-1])
        code="import sys\nfor line in "+repr(sequence)+":\n sys.stdout.buffer.write(line);sys.stdout.flush()\n if line.startswith(b'RSS '):\n  assert sys.stdin.buffer.readline()==b'CONTINUE\\n'\n"
        with tempfile.TemporaryDirectory() as directory:
            result=execute_worker([sys.executable,"-c",code],100,Path(directory)/"rss.json",rss=True)
            self.assertEqual(result["status"],"ok")
            self.assertEqual(result["report"],report)
            self.assertEqual(len(result["rss_observations"]),61)
            self.assertEqual(result["rss_observations"][0]["phase"],"baseline")
            self.assertEqual(result["rss_observations"][-1]["cycle"],29)
        for invalid in ([checkpoint("after_open",0)],[checkpoint("baseline",None),checkpoint("baseline",None)],sequence[:-2]+[lines[-1]],initial+sequence[:2]+sequence[2+len(initial):]):
            with self.assertRaises(RunnerError):
                capture=ProofCapture(100)
                for line in invalid:capture.feed(line)
                capture.finish()


class NativeAdmissionTests(unittest.TestCase):
    """Synthetic admission protocol (benchmarks/admission-plan.md); no performance claim."""
    def setUp(self):
        import os
        self.read,self.write=os.pipe()
        self.addCleanup(os.close,self.read)
        self.addCleanup(os.close,self.write)
        self.announced=[];self.events=[];self.frozen=True;self.replies={}
        self.directory=tempfile.TemporaryDirectory();self.addCleanup(self.directory.cleanup)
        self.admission=Admission(self.read,self.announce,lambda:self.frozen,self.events)

    def announce(self,line):
        import os
        # Models the parent: it reacts only to an observed announcement.
        self.announced.append(line)
        reply=self.replies.pop(line,None)
        if reply:
            os.write(self.write,reply.encode())

    def launch(self,label):
        _,lines,_=transcript(0)
        code="import sys; sys.stdout.buffer.write("+repr(b"".join(lines))+")"
        path=Path(self.directory.name)/(label+"-raw.json")
        return path,execute_worker([sys.executable,"-c",code],0,path)

    def await_line(self,coordinates):
        return "AWAIT_ADMISSION "+canonical(coordinates)

    def test_A01_A03_exact_permit_precedes_launch_and_held_follows_reaped_persisted_raw(self):
        import os
        first={"label":"warm-memory-00-1","run":1};second={"label":"warm-memory-00-2","run":2}
        self.replies[self.await_line(first)]="ADMIT "+canonical(first)+"\n"
        self.admission.admit(first)
        path,raw=self.launch("first")
        self.assertEqual(raw["status"],"ok")
        held=self.admission.hold("warm-memory-00-1",path,raw["pid"])
        with self.assertRaises(ChildProcessError):
            os.waitpid(raw["pid"],os.WNOHANG)
        self.assertEqual(held["raw_sha256"],hashlib.sha256(path.read_bytes()).hexdigest())
        self.assertEqual(held["worker_retirement"],"os.wait4 exact child reaped")
        self.assertIn("uncontrolled",held["continuity"])
        self.assertEqual(self.announced,[self.await_line(first),"BETWEEN_WORKERS "+canonical(held)])
        self.replies[self.await_line(second)]="ADMIT "+canonical(first)+"\n"
        with self.assertRaises(RunnerError):
            self.admission.admit(second)
        kinds=[event["event"] for event in self.events]
        self.assertEqual(kinds[:3],["admit","between_workers","rejected"])

    def test_A02_preissued_permit_is_rejected_before_the_held_announcement(self):
        first={"label":"warm-memory-00-1","run":1};second={"label":"warm-memory-00-2","run":2}
        self.replies[self.await_line(first)]="ADMIT "+canonical(first)+"\nADMIT "+canonical(second)+"\n"
        self.admission.admit(first)
        path,raw=self.launch("first")
        with self.assertRaises(RunnerError):
            self.admission.hold("warm-memory-00-1",path,raw["pid"])
        self.assertFalse(any(line.startswith("BETWEEN_WORKERS") for line in self.announced))

    def test_A02_input_before_first_announcement_is_preissued(self):
        import os
        first={"label":"warm-memory-00-1","run":1}
        os.write(self.write,("ADMIT "+canonical(first)+"\n").encode())
        with self.assertRaises(RunnerError):
            self.admission.admit(first)
        self.assertEqual(self.announced,[])

    def test_A04_A06_held_declaration_then_drift_blocks_and_new_exact_permit_admits(self):
        first={"label":"warm-memory-00-1","run":1};second={"label":"warm-memory-00-2","run":2}
        self.replies[self.await_line(first)]="ADMIT "+canonical(first)+"\n"
        self.admission.admit(first)
        path,raw=self.launch("first")
        held=self.admission.hold("warm-memory-00-1",path,raw["pid"])
        self.replies[self.await_line(second)]="DECLARE "+canonical({"work":"node --test http.test.mjs"})+"\nADMIT "+canonical(second)+"\n"
        self.frozen=False
        with self.assertRaises(RunnerError):
            self.admission.admit(second)
        declared=[event for event in self.events if event["event"]=="declare"]
        self.assertEqual(declared[0]["declaration"],{"work":"node --test http.test.mjs"})
        admitted=Admission(self.read,self.announce,lambda:True,self.events)
        admitted.held=held
        self.replies[self.await_line(second)]="ADMIT "+canonical(second)+"\n"
        admitted.admit(second)
        record=self.events[-1]
        self.assertEqual(record["event"],"admit")
        self.assertEqual(record["hold"],held)
        self.assertLessEqual(held["monotonic_ns"],record["monotonic_ns"])

    def test_A07_ok_run_status_requires_stdin_admission_at_argument_validation(self):
        import contextlib,io
        with contextlib.redirect_stderr(io.StringIO()),self.assertRaises(SystemExit):
            arguments(["--output","unused","--run-status","ok"])
        self.assertEqual(arguments(["--output","unused","--run-status","ok","--admission","stdin"]).admission,"stdin")
        self.assertEqual(arguments(["--output","unused"]).run_status,"contaminated")

    def test_A05_declaration_outside_hold_cancel_and_malformed_control_reject(self):
        first={"label":"warm-memory-00-1","run":1}
        for reply in ("DECLARE "+canonical({"work":"build"})+"\n","CANCEL "+canonical({"reason":"stop"})+"\n","ADMIT {\"run\":1}\n","PAUSE\n"):
            with self.subTest(reply=reply):
                self.announced.clear()
                self.replies[self.await_line(first)]=reply
                with self.assertRaises(RunnerError):
                    self.admission.admit(first)
        self.assertIn("cancelled",[event["event"] for event in self.events])


if __name__=="__main__":
    unittest.main()
