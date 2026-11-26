"""Synthetic protocol tests only: no browser/database timing measurements."""
import copy
import json
from pathlib import Path
import tempfile
import unittest
from browser_server import BrowserCollector
from run import RunnerError, canonical, proof_records

ROOT = Path(__file__).resolve().parents[1]


class BrowserControllerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="hoyadb-browser-controller-test-")
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        self.metadata = json.loads((ROOT / "benchmarks/browser-workloads.json").read_text())
        self.parent = json.loads((ROOT / "benchmarks/workloads.json").read_text())

    def collector(self, *, allow=True, metadata=None, next_wait=0.2):
        self.closure = ("a" * 64, "b" * 64)
        self.held = []
        return BrowserCollector(self.output, metadata or self.metadata, self.parent,
                                "a" * 64, "b" * 64, allow_measurements=allow, run_status="ok",
                                recheck=lambda: self.closure, on_held=self.held.append, next_wait_seconds=next_wait)

    def start(self, collector, engine, cohort, run, worker_id):
        collector.admit({"kind": "warm", "engine": engine, "cohort": cohort, "run": run, "worker_id": worker_id})
        return collector.start(engine, cohort, run, worker_id)

    def start_cold(self, collector, engine, operation, n, run, worker_id):
        collector.admit({"kind": "cold", "engine": engine, "operation": operation, "n": n, "run": run, "worker_id": worker_id})
        return collector.start_cold(engine, operation, n, run, worker_id)

    def test_quiet_gate_rejects_without_starting_a_run(self):
        with self.assertRaises(RunnerError):
            self.collector(allow=False).start("memory", 0, 1, "worker:1")
        self.assertEqual(list(self.output.iterdir()), [])

    def test_exact_metadata_and_coordinates_are_required(self):
        changed = copy.deepcopy(self.metadata)
        changed["cohorts"].pop()
        with self.assertRaises(RunnerError):
            self.collector(metadata=changed)
        collector = self.collector()
        for args in [("plaintext", 0, 1, "w"), ("memory", 51, 1, "w"),
                     ("memory", 0, 0, "w"), ("memory", 0, 1, "")]:
            with self.assertRaises(RunnerError):
                collector.admit(dict(zip(("engine", "cohort", "run", "worker_id"), args), kind="warm"))
            collector.recover(None, "synthetic invalid coordinate test")
            with self.assertRaises(RunnerError):
                collector.start(*args)

    def test_declared_parent_digest_and_state_must_match_actual_reviewed_source(self):
        for field, value in [("source_workload", {"sha256": "0" * 64}),
                             ("measurements", "already_started")]:
            changed = copy.deepcopy(self.metadata)
            changed[field] = value
            with self.assertRaises(RunnerError):
                self.collector(metadata=changed)

    def test_duplicate_run_and_worker_id_cannot_create_independent_runs(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        collector.finish(info["run_id"], self.document(info))
        for args in [("memory", 0, 1, "worker:2"), ("memory", 0, 2, "worker:1")]:
            with self.assertRaises(RunnerError):
                self.start(collector, *args)
            collector.recover(None, "synthetic duplicate identity test")
            with self.assertRaises(RunnerError):
                collector.start(*args)

    def test_bounded_independent_oracle_rejects_wrong_typed_data(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        collector.feed(info["run_id"], b"READY\nPROOF_BEGIN initial\n")
        wrong = next(iter(proof_records(100)))
        wrong["row"]["fields"]["ratio"]["kind"] = "integer"
        with self.assertRaises(RunnerError):
            collector.feed(info["run_id"], ("PROOF " + canonical(wrong) + "\n").encode())
        with self.assertRaises(RunnerError):
            collector.feed(info["run_id"], b"x" * (262144 + 1))
        with self.assertRaises(RunnerError):
            collector.feed("../foreign", b"READY\n")

    def proof(self, collector, key, n=100):
        collector.feed(key, b"READY\n")
        for phase in ["initial", "final"]:
            collector.feed(key, ("PROOF_BEGIN " + phase + "\n").encode())
            for record in proof_records(n):
                collector.feed(key, ("PROOF " + canonical(record) + "\n").encode())
            collector.feed(key, ("PROOF_END " + phase + "\n").encode())

    def document(self, info, run=1):
        return {"format": 1, "identity": info["identity"],
                "protocol": {"runs": 5, "warmups": 5, "samples": 30},
                "lifecycle": {"owner_closed": True, "worker_terminated": True, "removed_names": [], "pool_before": None, "pool_after": None, "canary_before_sha256": None, "canary_after_sha256": None},
                "runs": [{"run": run, "status": "ok", "samples": [
                    {"phase": phase, "index": index, "status": "ok", "correct": True, "value": 1}
                    for phase, count in [("warmup", 5), ("measured", 30)]
                    for index in range(count)]}]}

    def test_complete_proof_and_five_runs_are_required_for_summary(self):
        collector = self.collector()
        for run in range(1, 6):
            info = self.start(collector, "memory", 0, run, "worker:" + str(run))
            self.proof(collector, info["run_id"])
            result = collector.finish(info["run_id"], self.document(info, run))
            self.assertEqual(result["status"], "accepted")
            self.assertEqual(result["summary_ready"], run == 5)
        summary = list(self.output.glob("*.summary.json"))
        self.assertEqual(len(summary), 1)
        self.assertEqual(json.loads(summary[0].read_text())["sample_count"], 150)

    def cold_document(self, info, run):
        document = self.document(info, run)
        document['protocol'] = {'runs': 10, 'warmups': 0, 'samples': 1}
        document['runs'][0]['samples'] = [{'phase': 'measured', 'index': 0, 'status': 'ok', 'correct': True, 'value': 3, 'worker_creation_to_sdk_ready_ns': 5}]
        return document

    def test_cold_admission_uses_exact_declared_classes_sizes_and_ten_distinct_workers(self):
        collector = self.collector()
        for args in [('memory', 'reopen', 100, 1, 'x'), ('opfs-protected', 'upgrade', 100, 1, 'y'), ('memory', 'create', 0, 11, 'z')]:
            with self.assertRaises(RunnerError):
                self.start_cold(collector, *args)
            collector.recover(None, 'synthetic undeclared cold test')
            with self.assertRaises(RunnerError):
                collector.start_cold(*args)
        first = self.start_cold(collector, 'memory', 'create', 0, 10, 'cold:10')
        self.assertEqual(first['identity']['endpoint'], 'sdk-open-invocation-to-decoded-ready')
        self.assertEqual(first['cohort'], {'operation': 'create', 'N': 0, 'Q': 0, 'observers': 0})
        self.proof(collector, first['run_id'], 0)
        collector.finish(first['run_id'], self.cold_document(first, 10))
        with self.assertRaises(RunnerError):
            self.start_cold(collector, 'memory', 'create', 0, 1, 'cold:10')

    def test_cold_metadata_cannot_add_a_class_or_change_reviewed_sizes(self):
        for field, value in [('classes', ['browser-sdk/memory']), ('N', 100)]:
            changed = copy.deepcopy(self.metadata)
            changed['cold'][1][field] = value
            with self.assertRaises(RunnerError):
                self.collector(metadata=changed)

    def test_complete_cold_proof_and_ten_runs_are_required_for_a_separate_summary(self):
        collector = self.collector()
        for run in range(1, 11):
            info = self.start_cold(collector, 'memory', 'create', 0, run, 'cold:' + str(run))
            self.proof(collector, info['run_id'], 0)
            result = collector.finish(info['run_id'], self.cold_document(info, run))
            self.assertEqual(result['summary_ready'], run == 10)
        summary = list(self.output.glob('*.summary.json'))
        self.assertEqual(len(summary), 1)
        self.assertEqual(json.loads(summary[0].read_text())['sample_count'], 10)

    def test_cold_missing_or_inconsistent_outer_endpoint_is_rejected_with_original_raw_retained(self):
        collector = self.collector()
        info = self.start_cold(collector, 'memory', 'create', 0, 1, 'cold:1')
        self.proof(collector, info['run_id'], 0)
        document = self.cold_document(info, 1)
        document['runs'][0]['samples'][0]['worker_creation_to_sdk_ready_ns'] = 2
        with self.assertRaises(RunnerError):
            collector.finish(info['run_id'], document)
        self.assertTrue((self.output / (info['run_id'] + '.raw.json')).is_file())
        self.assertFalse(list(self.output.glob('*.summary.json')))

    def test_successful_samples_cannot_be_accepted_without_closed_worker_cleanup(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        document = self.document(info)
        document["lifecycle"]["worker_terminated"] = False
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], document)
        self.assertEqual(list(self.output.glob("*.json")), [self.output / (info["run_id"] + ".raw.json")])

    def test_protected_cleanup_cannot_claim_different_canary_or_foreign_names(self):
        collector = self.collector()
        info = self.start(collector, "opfs-protected", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        document = self.document(info)
        document["lifecycle"] = {"owner_closed": True, "worker_terminated": True,
            "removed_names": ["unrelated.sqlite"], "pool_before": [24, 2], "pool_after": [24, 2],
            "canary_before_sha256": "a" * 64, "canary_after_sha256": "b" * 64}
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], document)
        self.assertEqual(list(self.output.glob("*.summary.json")), [])

    def test_exact_protected_cleanup_accepts_matching_owned_files_pool_and_canary(self):
        collector = self.collector()
        info = self.start(collector, "opfs-protected", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        document = self.document(info)
        self.assertEqual(len(info["owned_names"]), 2)
        self.assertTrue(all(name.startswith("phase-seven-sdk-owned-bench-") for name in info["owned_names"]))
        document["lifecycle"] = {"owner_closed": True, "worker_terminated": True,
            "removed_names": info["owned_names"], "pool_before": [24, 2], "pool_after": [24, 2],
            "canary_before_sha256": "a" * 64, "canary_after_sha256": "a" * 64}
        self.assertEqual(collector.finish(info["run_id"], document)["status"], "accepted")

    def test_boolean_sample_index_cannot_pass_integer_protocol(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        document = self.document(info)
        document["runs"][0]["samples"][1]["index"] = True
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], document)

    def test_identity_failure_keeps_raw_sample_without_acceptance(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        sample = {"phase": "measured", "index": 0, "status": "error",
                  "correct": False, "error": "BackendFailure"}
        collector.sample(info["run_id"], sample)
        stored = list(self.output.glob("*.samples.jsonl"))
        self.assertEqual(len(stored), 1)
        self.assertEqual(json.loads(stored[0].read_text()), sample)
        doc = self.document(info)
        doc["identity"]["artifact_sha256"] = "c" * 64
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], doc)
        self.assertEqual(list(self.output.glob("*.summary.json")), [])

    # Between-worker admission (benchmarks/admission-plan.md A01-A07); synthetic protocol only.
    def control(self):
        return [json.loads(line) for line in (self.output / "control.jsonl").read_text().splitlines()]

    def finished(self, collector, run=1, worker="worker:1"):
        info = self.start(collector, "memory", 0, run, worker)
        self.proof(collector, info["run_id"])
        collector.finish(info["run_id"], self.document(info, run))
        return info

    def test_A01_no_permit_rejects_before_any_run_state_and_one_exact_permit_admits_one_run(self):
        collector = self.collector()
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 1, "worker:1")
        with self.assertRaises(RunnerError):
            collector.start_cold("memory", "create", 0, 1, "cold:1")
        self.assertEqual((collector.runs, collector.workers), ({}, set()))
        self.assertFalse(any(path.suffix == ".json" for path in self.output.iterdir()))
        permit = collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"}, {"visibility": "visible"})
        self.assertEqual(permit["coordinates"], {"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"})
        self.assertEqual(collector.start("memory", 0, 1, "worker:1")["run_id"], "memory.c00.r1")
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 2, "worker:2")
        self.assertEqual(list(collector.runs), ["memory.c00.r1"])

    def test_A02_wrong_duplicate_stale_or_preissued_permits_reject(self):
        collector = self.collector()
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"})
        for args in [("memory", 0, 2, "worker:1"), ("memory", 0, 1, "worker:2"), ("opfs-protected", 0, 1, "worker:1")]:
            with self.assertRaises(RunnerError):
                collector.start(*args)
        with self.assertRaises(RunnerError):
            collector.start_cold("memory", "create", 0, 1, "worker:1")
        info = collector.start("memory", 0, 1, "worker:1")
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        self.proof(collector, info["run_id"])
        collector.finish(info["run_id"], self.document(info))
        self.assertEqual(len(self.held), 1)
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        collector.recover(None, "pre-issued permit observed")
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:9"})
        collector.recover(None, "stale permit observed")
        for malformed in [{"kind": "warm", "engine": "memory", "cohort": 0, "run": 2}, {"kind": "pause"}, ["warm"],
                          {"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2", "extra": 1}]:
            with self.assertRaises(RunnerError):
                collector.admit(malformed)
            collector.recover(None, "malformed control input")
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 3, "worker_id": "worker:3"})

    def test_A03_between_workers_only_after_persisted_raw_and_validated_cleanup(self):
        import hashlib
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        self.assertIsNone(collector.between_workers())
        document = self.document(info)
        document["lifecycle"]["worker_terminated"] = False
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], document)
        self.assertIsNone(collector.between_workers())
        self.assertEqual(self.held, [])
        self.assertTrue((self.output / "memory.c00.r1.raw.json").is_file())
        accepted = self.collector()
        self.output = self.output / "second"
        accepted.output = self.output
        info = self.start(accepted, "memory", 0, 1, "worker:1")
        self.proof(accepted, info["run_id"])
        self.assertIsNone(accepted.between_workers())
        accepted.finish(info["run_id"], self.document(info))
        held = accepted.between_workers()
        self.assertEqual(self.held, [held])
        self.assertEqual(held["run_id"], "memory.c00.r1")
        self.assertEqual(held["raw_sha256"], hashlib.sha256((self.output / "memory.c00.r1.raw.json").read_bytes()).hexdigest())
        self.assertEqual(held["accepted_sha256"], hashlib.sha256((self.output / "memory.c00.r1.json").read_bytes()).hexdigest())
        self.assertEqual(held["worker_retirement"], "browser Worker.terminate() request; not OS process reaping")
        self.assertIn("uncontrolled", held["continuity"])

    def test_A04_held_state_requires_new_permit_and_frozen_recheck_outside_worker_clocks(self):
        collector = self.collector()
        self.finished(collector)
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 2, "worker:2")
        self.closure = ("c" * 64, "b" * 64)
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        self.closure = ("a" * 64, "b" * 64)
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        collector.recover(None, "frozen closure drift investigated")
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"}, {"visibility": "visible"})
        collector.start("memory", 0, 2, "worker:2")
        events = self.control()
        held = next(event for event in events if event["event"] == "between_workers")
        admitted = [event for event in events if event["event"] == "admit"][-1]
        started = [event for event in events if event["event"] == "start"][-1]
        self.assertLessEqual(held["monotonic_ns"], admitted["monotonic_ns"])
        self.assertEqual(admitted["hold_started_monotonic_ns"], held["monotonic_ns"])
        self.assertEqual(admitted["environment"], {"visibility": "visible"})
        self.assertLessEqual(admitted["monotonic_ns"], started["monotonic_ns"])
        self.assertLessEqual(started["monotonic_ns"], collector.runs["memory.c00.r2"]["started_monotonic_ns"])
        self.assertIn("drift", " ".join(event.get("reason", "") for event in events if event["event"] == "rejected"))

    def test_A05_failed_run_blocks_admission_until_explicit_recovery_and_cancel_is_terminal(self):
        collector = self.collector()
        info = self.start(collector, "memory", 0, 1, "worker:1")
        self.proof(collector, info["run_id"])
        document = self.document(info)
        document["runs"][0]["status"] = "failed"
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], document)
        raw = (self.output / "memory.c00.r1.raw.json").read_bytes()
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        with self.assertRaises(RunnerError):
            collector.recover("memory.c00.r9", "wrong run")
        collector.recover("memory.c00.r1", "failed raw retained for review")
        self.assertIsNone(collector.between_workers())
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        collector.cancel("operator stopped experiment")
        for operation in (lambda: collector.start("memory", 0, 2, "worker:2"),
                          lambda: collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 3, "worker_id": "worker:3"}),
                          lambda: collector.recover(None, "resume")):
            with self.assertRaises(RunnerError):
                operation()
        self.assertEqual((self.output / "memory.c00.r1.raw.json").read_bytes(), raw)
        kinds = [event["event"] for event in self.control()]
        for kind in ("run_failed", "recovered", "cancelled"):
            self.assertIn(kind, kinds)

    def test_A06_competing_work_is_declared_only_while_held_and_contaminates_an_active_run(self):
        collector = self.collector()
        with self.assertRaises(RunnerError):
            collector.declare({"work": "before any held state"})
        collector.recover(None, "declaration outside held state")
        self.finished(collector)
        collector.declare({"work": "node --test tools/javascript/benchmark/http.test.mjs"})
        info = self.start(collector, "memory", 0, 2, "worker:2")
        with self.assertRaises(RunnerError):
            collector.declare({"work": "competing build during active worker"})
        self.proof(collector, info["run_id"])
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], self.document(info, 2))
        self.assertTrue((self.output / "memory.c00.r2.raw.json").is_file())
        self.assertFalse((self.output / "memory.c00.r2.json").exists())
        declared = [event for event in self.control() if event["event"] == "declare"]
        self.assertEqual(declared[0]["declaration"], {"work": "node --test tools/javascript/benchmark/http.test.mjs"})
        self.assertEqual(declared[0]["state"], "held")

    def test_A07_measurement_disabled_controller_cannot_admit_or_fabricate(self):
        collector = self.collector(allow=False)
        with self.assertRaises(RunnerError):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"})
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 1, "worker:1")
        self.assertFalse(any(path.suffix == ".json" for path in self.output.iterdir()))


    def test_contaminated_default_retains_accepted_raw_runs_without_any_summary(self):
        collector = BrowserCollector(self.output, self.metadata, self.parent, "a" * 64, "b" * 64, allow_measurements=True,
                                     recheck=lambda: ("a" * 64, "b" * 64))
        self.assertEqual(collector.run_status, "contaminated")
        for run in range(1, 6):
            collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": run, "worker_id": "worker:" + str(run)})
            info = collector.start("memory", 0, run, "worker:" + str(run))
            self.proof(collector, info["run_id"])
            self.assertEqual(collector.finish(info["run_id"], self.document(info, run)), {"status": "accepted_contaminated", "summary_ready": False})
            self.assertEqual(json.loads((self.output / (info["run_id"] + ".json")).read_text())["run_status"], "contaminated")
        self.assertEqual(list(self.output.glob("*.summary.json")), [])
        with self.assertRaises(RunnerError):
            BrowserCollector(self.output, self.metadata, self.parent, "a" * 64, "b" * 64, run_status="quiet")


    # Untimed resource class (browser-plan resource classes); synthetic protocol only.
    def start_resources(self, collector, engine="memory", worker="resources:1"):
        collector.admit({"kind": "resources", "engine": engine, "run": 1, "worker_id": worker})
        return collector.start_resources(engine, 1, worker)

    def record(self, phase, cycle, bytes_=131072):
        return {"phase": phase, "cycle": cycle, "ordinary_worker_linear_bytes": None, "harness_worker_linear_bytes": bytes_,
                "js_heap_bytes": None, "ua_specific": {"status": "unknown", "reason": "unavailable"}}

    def proof_phase(self, collector, key, phase):
        collector.feed(key, ("PROOF_BEGIN " + phase + "\n").encode())
        for record in proof_records(100):
            collector.feed(key, ("PROOF " + canonical(record) + "\n").encode())
        collector.feed(key, ("PROOF_END " + phase + "\n").encode())

    def resource_run(self, collector, key, until=61):
        sent = 0
        def send(record):
            nonlocal sent
            if sent < until:
                self.assertEqual(collector.checkpoint(key, record), {"reply": "CONTINUE"})
            sent += 1
        send(self.record("baseline", None))
        for cycle in range(30):
            if cycle in (0, 29):
                self.proof_phase(collector, key, "initial" if cycle == 0 else "final")
            send(self.record("after_open", cycle, 131072 + 65536 * min(cycle, 3)))
            send(self.record("after_close", cycle, 131072 + 65536 * min(cycle, 3)))

    def resource_document(self, info, lifecycle=None):
        return {"format": 1, "identity": info["identity"], "protocol": {"cycles": 30, "checkpoints": 61},
                "lifecycle": lifecycle or self.document(info)["lifecycle"], "runs": [{"run": 1, "status": "ok"}]}

    def test_P03_resource_class_has_declared_store_semantics_and_untimed_identity(self):
        collector = self.collector()
        memory = self.start_resources(collector)
        self.assertEqual(memory["run_id"], "resources.memory.r1")
        self.assertEqual(memory["identity"]["unit"], "bytes")
        self.assertEqual(memory["identity"]["endpoint"], "untimed-open-observe-close-resources")
        self.assertEqual(memory["cohort"], {"operation": "retained-resources", "N": 100, "Q": 0, "observers": 1, "mode": "membership",
                                            "sharing": "equivalent", "store": "fresh-memory-per-cycle"})
        self.assertEqual(memory["owned_names"], [])
        other = self.collector()
        other.output = self.output / "protected"
        protected = self.start_resources(other, "opfs-protected")
        self.assertEqual(protected["cohort"]["store"], "same-owned-store-reopen")
        self.assertEqual(len(protected["owned_names"]), 2)
        self.assertTrue(protected["owned_names"][0].startswith("phase-seven-sdk-owned-bench-resources-r1-"))
        for coordinates in ({"kind": "resources", "engine": "memory", "run": 2, "worker_id": "r:2"}, {"kind": "resources", "engine": "plain", "run": 1, "worker_id": "r:3"}):
            third = self.collector()
            third.output = self.output / str(len(coordinates["worker_id"])) / coordinates["engine"]
            with self.assertRaises(RunnerError):
                third.admit(coordinates)

    def test_P03_exact_61_acknowledged_checkpoints_with_first_final_proof_are_persisted_raw_and_accepted(self):
        collector = self.collector()
        info = self.start_resources(collector)
        self.resource_run(collector, info["run_id"])
        stored = (self.output / "resources.memory.r1.checkpoints.jsonl").read_text().splitlines()
        self.assertEqual(len(stored), 61)
        result = collector.finish(info["run_id"], self.resource_document(info))
        self.assertEqual(result, {"status": "accepted", "summary_ready": False})
        saved = json.loads((self.output / "resources.memory.r1.json").read_text())
        self.assertEqual(saved["metric"]["baseline_harness_worker_linear_bytes"], 131072)
        self.assertEqual(saved["metric"]["final_post_close_harness_worker_linear_bytes"], 131072 + 65536 * 3)
        self.assertEqual(saved["metric"]["retained_delta_harness_worker_linear_bytes"], 65536 * 3)
        self.assertEqual(saved["metric"]["ordinary_worker_linear_bytes"], None)
        self.assertIn("not an allocator leak conclusion", saved["metric"]["interpretation"])
        self.assertEqual(len(saved["checkpoints"]), 61)
        self.assertEqual(list(self.output.glob("*.summary.json")), [])
        self.assertIsNotNone(collector.between_workers())

    def test_P03_wrong_sequence_fabricated_metric_misplaced_proof_or_missing_checkpoints_reject(self):
        bad = [
            [self.record("after_open", 0)],
            [self.record("baseline", None), self.record("after_open", 0)],
            [self.record("baseline", None) | {"ordinary_worker_linear_bytes": 65536}],
            [self.record("baseline", None) | {"js_heap_bytes": 0}],
            [self.record("baseline", None, 1000)],
            [self.record("baseline", None) | {"ua_specific": {"status": "observed", "value": {"bytes": -1, "breakdown": []}}}],
            [self.record("baseline", None) | {"extra": 1}],
        ]
        for index, records in enumerate(bad):
            with self.subTest(index=index):
                collector = self.collector()
                collector.output = self.output / str(index)
                info = self.start_resources(collector)
                with self.assertRaises(RunnerError):
                    for record in records:
                        collector.checkpoint(info["run_id"], record)
        collector = self.collector()
        collector.output = self.output / "oversized-profiler"
        info = self.start_resources(collector)
        oversized = self.record("baseline", None) | {"ua_specific": {"status": "unknown", "reason": "exceeds_bounded_transport", "serialized_bytes": 300000}}
        self.assertEqual(collector.checkpoint(info["run_id"], oversized), {"reply": "CONTINUE"})
        collector = self.collector()
        collector.output = self.output / "incomplete"
        info = self.start_resources(collector)
        self.resource_run(collector, info["run_id"], until=60)
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], self.resource_document(info))
        self.assertTrue((collector.output / "resources.memory.r1.raw.json").is_file())
        self.assertIsNone(collector.between_workers())

    def test_P03_protected_resource_acceptance_requires_final_owned_cleanup_canary_and_pool(self):
        collector = self.collector()
        info = self.start_resources(collector, "opfs-protected")
        self.resource_run(collector, info["run_id"])
        lifecycle = {"owner_closed": True, "worker_terminated": True, "removed_names": info["owned_names"], "pool_before": [24, 0],
                     "pool_after": [24, 1], "canary_before_sha256": "a" * 64, "canary_after_sha256": "a" * 64}
        with self.assertRaises(RunnerError):
            collector.finish(info["run_id"], self.resource_document(info, lifecycle))


    # Page permit transport (admission-plan reviewed amendment N01-N07); synthetic protocol only.
    WARM = {"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"}

    def waiting(self, collector):
        import threading
        result = {}
        thread = threading.Thread(target=lambda: result.setdefault("value", collector.next_permit()))
        thread.start()
        for _ in range(400):
            if collector.waiter:
                break
            import time
            time.sleep(0.005)
        self.assertTrue(collector.waiter)
        return thread, result

    def test_N01_no_permit_returns_bounded_timeout_without_admission_or_state(self):
        import time
        collector = self.collector(next_wait=0.05)
        started = time.monotonic()
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "timeout"})
        self.assertLess(time.monotonic() - started, 2)
        self.assertEqual((collector.runs, collector.workers, collector.permit, collector.state), ({}, set(), None, "idle"))
        self.assertFalse(any(event["event"] in ("admit", "claim") for event in self.control()) if (self.output / "control.jsonl").exists() else False)

    def test_N02_admit_before_request_claims_once_and_never_redelivers(self):
        collector = self.collector()
        collector.admit(dict(self.WARM))
        self.assertEqual(collector.next_permit(), {"status": "permit", "coordinates": self.WARM})
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "already_claimed"})
        self.assertEqual([event["event"] for event in self.control()].count("claim"), 1)
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 2, "worker:1")
        self.assertIsNotNone(collector.permit)
        self.assertEqual(collector.start("memory", 0, 1, "worker:1")["run_id"], "memory.c00.r1")

    def test_N02_request_before_admit_wakes_and_second_waiter_is_rejected_without_waiting(self):
        import time
        collector = self.collector(next_wait=5)
        thread, result = self.waiting(collector)
        started = time.monotonic()
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "already_claimed"})
        self.assertLess(time.monotonic() - started, 1)
        collector.admit(dict(self.WARM))
        thread.join(5)
        self.assertEqual(result["value"], {"status": "permit", "coordinates": self.WARM})
        self.assertFalse(collector.waiter)

    def test_N03_active_worker_rejects_immediately_and_cannot_claim_a_later_permit(self):
        import time
        collector = self.collector(next_wait=5)
        self.start(collector, "memory", 0, 1, "worker:1")
        started = time.monotonic()
        with self.assertRaises(RunnerError):
            collector.next_permit()
        self.assertLess(time.monotonic() - started, 1)
        self.assertFalse(collector.waiter)
        self.assertNotIn("claim", [event["event"] for event in self.control()])

    def test_N05_waiter_wakes_on_cancel_and_on_shutdown(self):
        collector = self.collector(next_wait=5)
        thread, result = self.waiting(collector)
        collector.cancel("operator stop")
        thread.join(2)
        self.assertEqual(result["value"], {"status": "none", "reason": "cancelled"})
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "cancelled"})
        other = self.collector(next_wait=5)
        other.output = self.output / "shutdown"
        thread, result = self.waiting(other)
        other.shutdown()
        thread.join(2)
        self.assertFalse(thread.is_alive())
        self.assertEqual(result["value"], {"status": "none", "reason": "cancelled"})

    def test_N06_claimed_unstarted_permit_stays_until_revoke_which_blocks_until_recover(self):
        collector = self.collector()
        collector.admit(dict(self.WARM))
        self.assertEqual(collector.next_permit()["status"], "permit")
        self.assertIsNotNone(collector.permit)
        collector.revoke("page disconnected before start")
        self.assertIsNone(collector.permit)
        with self.assertRaises(RunnerError):
            collector.start("memory", 0, 1, "worker:1")
        with self.assertRaises(RunnerError):
            collector.admit(dict(self.WARM))
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "blocked"})
        collector.recover(None, "revocation acknowledged")
        collector.admit(dict(self.WARM))
        self.assertEqual(collector.next_permit()["status"], "permit")
        collector.start("memory", 0, 1, "worker:1")
        with self.assertRaises(RunnerError):
            collector.revoke("during active worker")
        self.assertIn("revoked", [event["event"] for event in self.control()])
        with self.assertRaises(RunnerError):
            self.collector().revoke("no permit")

    def test_N07_drift_between_admit_and_consumption_rejects_before_any_worker_and_blocks(self):
        for transported in (True, False):
            with self.subTest(transported=transported):
                collector = self.collector()
                collector.output = self.output / str(transported)
                collector.admit(dict(self.WARM))
                if transported:
                    self.assertEqual(collector.next_permit()["status"], "permit")
                self.closure = ("c" * 64, "b" * 64)
                with self.assertRaises(RunnerError):
                    collector.start("memory", 0, 1, "worker:1")
                self.assertEqual((collector.runs, collector.workers, collector.active), ({}, set(), None))
                self.assertTrue(collector.blocked)
                self.closure = ("a" * 64, "b" * 64)
                with self.assertRaises(RunnerError):
                    collector.admit({**self.WARM, "run": 2, "worker_id": "worker:2"})


    # DEF-0023: a waiter must not survive a direct start that wins the collector lock.
    def blocked_waiter(self, collector):
        """Register one waiter; the caller holds collector.lock to order ADMIT/start before its wakeup."""
        import threading
        outcome = {}
        def wait():
            try:
                outcome["value"] = collector.next_permit()
            except RunnerError as error:
                outcome["error"] = str(error)
        thread = threading.Thread(target=wait)
        thread.start()
        registered = threading.Event()
        for _ in range(2000):
            with collector.lock:
                if collector.waiter:
                    registered.set()
                    break
            thread.join(0.001)
        self.assertTrue(registered.is_set())
        return thread, outcome

    def test_DEF0023_waiter_retires_when_a_direct_start_wins_the_lock_for_each_start_route(self):
        routes = {
            "warm": ({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"}, lambda c: c.start("memory", 0, 1, "worker:1")),
            "cold": ({"kind": "cold", "engine": "memory", "operation": "create", "n": 0, "run": 1, "worker_id": "cold:1"}, lambda c: c.start_cold("memory", "create", 0, 1, "cold:1")),
            "resources": ({"kind": "resources", "engine": "memory", "run": 1, "worker_id": "resources:1"}, lambda c: c.start_resources("memory", 1, "resources:1")),
        }
        for name, (coordinates, start) in routes.items():
            with self.subTest(route=name):
                collector = self.collector(next_wait=5)
                collector.output = self.output / name
                thread, outcome = self.blocked_waiter(collector)
                with collector.lock:
                    collector.admit(dict(coordinates))
                    start(collector)
                thread.join(2)
                self.assertFalse(thread.is_alive())
                self.assertEqual(outcome, {"error": "next permit requested during an active worker"})
                self.assertFalse(collector.waiter)
                self.assertFalse(collector.claimed)
                events = [json.loads(line)["event"] for line in (collector.output / "control.jsonl").read_text().splitlines()]
                self.assertNotIn("claim", events)

    def test_DEF0023_waiter_retires_even_when_start_and_completion_both_win_and_cannot_claim_a_later_permit(self):
        collector = self.collector(next_wait=5)
        thread, outcome = self.blocked_waiter(collector)
        with collector.lock:
            info = self.start(collector, "memory", 0, 1, "worker:1")
            self.proof(collector, info["run_id"])
            collector.finish(info["run_id"], self.document(info))
            self.assertIsNone(collector.active)
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        thread.join(2)
        self.assertFalse(thread.is_alive())
        self.assertEqual(outcome, {"error": "next permit requested during an active worker"})
        self.assertFalse(collector.waiter)
        self.assertNotIn("claim", [event["event"] for event in self.control()])
        self.assertEqual(collector.next_permit(), {"status": "permit", "coordinates": {"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"}})

    def test_DEF0023_retiring_waiter_never_clears_a_newer_waiter_registration(self):
        collector = self.collector(next_wait=5)
        thread, outcome = self.blocked_waiter(collector)
        with collector.lock:
            info = self.start(collector, "memory", 0, 1, "worker:1")
            self.proof(collector, info["run_id"])
            collector.finish(info["run_id"], self.document(info))
        thread.join(2)
        self.assertEqual(outcome, {"error": "next permit requested during an active worker"})
        newer, result = self.blocked_waiter(collector)
        self.assertEqual(collector.next_permit(), {"status": "none", "reason": "already_claimed"})
        collector.admit({"kind": "warm", "engine": "memory", "cohort": 0, "run": 2, "worker_id": "worker:2"})
        newer.join(2)
        self.assertEqual(result["value"]["status"], "permit")


    # DEF-0029: start preflight must complete before the permit is consumed.
    ROUTES = {
        "warm": ({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"}, "memory.c00.r1",
                 lambda c, run, worker: c.start("memory", 0, run, worker)),
        "cold": ({"kind": "cold", "engine": "memory", "operation": "create", "n": 0, "run": 1, "worker_id": "worker:1"}, "cold.memory.create.n0.r1",
                 lambda c, run, worker: c.start_cold("memory", "create", 0, run, worker)),
        "resources": ({"kind": "resources", "engine": "memory", "run": 1, "worker_id": "worker:1"}, "resources.memory.r1",
                      lambda c, run, worker: c.start_resources("memory", run, worker)),
    }

    def state_of(self, collector):
        return {"permit": collector.permit, "state": collector.state, "starts": collector.starts, "runs": dict(collector.runs),
                "workers": set(collector.workers), "active": collector.active, "claimed": collector.claimed, "blocked": list(collector.blocked)}

    def events(self, collector):
        path = collector.output / "control.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def conflicted(self, name):
        collector = self.collector()
        collector.output = self.output / name
        coordinates, run_id, start = self.ROUTES[name]
        collector.admit(dict(coordinates))
        conflict = collector.output / (run_id + ".json")
        conflict.write_bytes(b'{"existing":"evidence"}\n')
        return collector, coordinates, run_id, start, conflict

    def test_DEF0029_P01_existing_evidence_after_admit_rejects_without_consuming_the_permit(self):
        for name in self.ROUTES:
            with self.subTest(route=name):
                collector, coordinates, run_id, start, conflict = self.conflicted(name)
                before = self.state_of(collector)
                with self.assertRaises(RunnerError):
                    start(collector, 1, "worker:1")
                self.assertEqual(self.state_of(collector), before)
                self.assertEqual((collector.state, collector.blocked), ("admitted", []))
                self.assertEqual(conflict.read_bytes(), b'{"existing":"evidence"}\n')
                kinds = [event["event"] for event in self.events(collector)]
                self.assertNotIn("start", kinds)
                self.assertEqual(kinds[-1], "rejected")

    def test_DEF0029_P02_revoke_recover_then_a_different_identity_starts_exactly_once(self):
        for name in self.ROUTES:
            with self.subTest(route=name):
                collector, coordinates, run_id, start, conflict = self.conflicted(name)
                with self.assertRaises(RunnerError):
                    start(collector, 1, "worker:1")
                collector.revoke("existing evidence for " + run_id)
                collector.recover(None, "conflict preserved; new identity")
                # Resource runs are declared as run 1 only, so its different identity is the other engine.
                fresh, begin = (({**coordinates, "engine": "opfs-protected", "worker_id": "worker:2"},
                                 lambda: collector.start_resources("opfs-protected", 1, "worker:2")) if name == "resources"
                                else ({**coordinates, "run": 2, "worker_id": "worker:2"}, lambda: start(collector, 2, "worker:2")))
                collector.admit(fresh)
                info = begin()
                self.assertEqual((collector.active, collector.starts, collector.state), (info["run_id"], 1, "active"))
                self.assertNotEqual(info["run_id"], run_id)
                with self.assertRaises(RunnerError):
                    begin()
                self.assertEqual(collector.starts, 1)
                self.assertEqual(conflict.read_bytes(), b'{"existing":"evidence"}\n')
                self.assertEqual([event["event"] for event in self.events(collector)].count("start"), 1)

    def test_DEF0029_P03_mismatch_and_drift_keep_their_semantics_and_drift_wins_over_existing_evidence(self):
        for name in self.ROUTES:
            with self.subTest(route=name, case="mismatch"):
                collector = self.collector()
                collector.output = self.output / ("mismatch-" + name)
                coordinates, run_id, start = self.ROUTES[name]
                collector.admit(dict(coordinates))
                before = self.state_of(collector)
                with self.assertRaises(RunnerError):
                    start(collector, 2, "worker:1")
                self.assertEqual(self.state_of(collector), before)
            for conflict in (False, True):
                with self.subTest(route=name, case="drift", conflict=conflict):
                    collector = self.collector()
                    collector.output = self.output / ("drift-" + name + str(conflict))
                    collector.admit(dict(coordinates))
                    if conflict:
                        (collector.output / (run_id + ".json")).write_bytes(b"conflict")
                    self.closure = ("c" * 64, "b" * 64)
                    with self.assertRaises(RunnerError) as caught:
                        start(collector, 1, "worker:1")
                    self.assertIn("drift", str(caught.exception))
                    self.assertEqual((collector.permit, collector.runs, collector.workers, collector.active, collector.starts),
                                     (None, {}, set(), None, 0))
                    self.assertTrue(collector.blocked)
                    self.assertNotEqual(collector.state, "active")
                    self.closure = ("a" * 64, "b" * 64)



class BrowserHTTPTests(unittest.TestCase):
    def setUp(self):
        import threading
        from browser_server import create_server
        self.temp = tempfile.TemporaryDirectory(prefix="hoyadb-browser-http-test-")
        self.addCleanup(self.temp.cleanup)
        root = Path(self.temp.name)
        metadata = json.loads((ROOT / "benchmarks/browser-workloads.json").read_text())
        parent = json.loads((ROOT / "benchmarks/workloads.json").read_text())
        collector = BrowserCollector(root / "evidence", metadata, parent, "a" * 64, "b" * 64)
        self.server = create_server(root, collector)
        self.addCleanup(self.server.server_close)
        self.addCleanup(self.server.shutdown)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.url = "http://127.0.0.1:" + str(self.server.server_port)

    def test_metadata_served_with_isolation_and_quiet_gate_closed(self):
        from urllib.request import urlopen
        with urlopen(self.url + "/benchmark/meta") as response:
            value = json.loads(response.read())
            self.assertEqual(value["metadata"]["fixture"], "TC-0036-A")
            self.assertFalse(value["measurements_allowed"])
            self.assertEqual(response.headers["Cross-Origin-Opener-Policy"], "same-origin")
            self.assertEqual(response.headers["Cross-Origin-Embedder-Policy"], "require-corp")

    def test_quiet_start_is_static_rejection_and_large_body_rejects(self):
        from urllib.request import Request, urlopen
        from urllib.error import HTTPError
        body = json.dumps({"engine": "memory", "cohort": 0, "run": 1, "worker_id": "w"}).encode()
        with self.assertRaises(HTTPError) as caught:
            urlopen(Request(self.url + "/benchmark/start", data=body))
        self.assertEqual(caught.exception.code, 409)
        self.assertEqual(json.loads(caught.exception.read()), {"ok": False, "error": "InvalidBenchmarkRequest"})
        caught.exception.close()
        with self.assertRaises(HTTPError) as caught:
            urlopen(Request(self.url + "/benchmark/lines?run=x", data=b"x" * 262145))
        self.assertEqual(caught.exception.code, 413)
        caught.exception.close()

    def test_A01_page_has_no_admission_route(self):
        from urllib.request import Request, urlopen
        from urllib.error import HTTPError
        body = json.dumps({"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "w"}).encode()
        for route in ("/benchmark/admit", "/benchmark/recover?run=x"):
            with self.assertRaises(HTTPError) as caught:
                urlopen(Request(self.url + route, data=body))
            self.assertEqual(caught.exception.code, 409)
            caught.exception.close()

    def admitted_server(self, wait=0.2):
        import threading
        from browser_server import create_server
        root = Path(self.temp.name)
        metadata = json.loads((ROOT / "benchmarks/browser-workloads.json").read_text())
        parent = json.loads((ROOT / "benchmarks/workloads.json").read_text())
        collector = BrowserCollector(root / "admitted", metadata, parent, "a" * 64, "b" * 64, allow_measurements=True,
                                     recheck=lambda: ("a" * 64, "b" * 64), next_wait_seconds=wait)
        server = create_server(root, collector)
        self.addCleanup(server.server_close)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        return collector, server, thread, "http://127.0.0.1:" + str(server.server_port)

    def post(self, url, body, headers=None):
        from urllib.request import Request, urlopen
        from urllib.error import HTTPError
        try:
            with urlopen(Request(url, data=body, headers=headers or {}), timeout=10) as response:
                return response.status, json.loads(response.read())
        except HTTPError as error:
            with error:
                return error.code, None

    def test_N04_next_route_is_exact_post_and_never_issues_or_consumes(self):
        from urllib.request import urlopen
        from urllib.error import HTTPError
        collector, server, thread, url = self.admitted_server()
        self.addCleanup(server.shutdown)
        coordinates = {"kind": "warm", "engine": "memory", "cohort": 0, "run": 1, "worker_id": "worker:1"}
        with self.assertRaises(HTTPError) as caught:
            urlopen(url + "/benchmark/next", timeout=10)
        self.assertNotEqual(caught.exception.code, 200)
        caught.exception.close()
        for body, path, headers in [(b'{"a":1}', "/benchmark/next", None), (b"{}", "/benchmark/next?x=1", None),
                                    (b"{}", "/benchmark/next", {"Origin": "http://evil.example"}), (b" {}", "/benchmark/next", None),
                                    (b"{" + b" " * 300 + b"}", "/benchmark/next", None)]:
            self.assertNotEqual(self.post(url + path, body, headers)[0], 200, (body[:10], path, headers))
        self.assertIsNone(collector.permit)
        self.assertEqual(self.post(url + "/benchmark/next", b"{}"), (200, {"status": "none", "reason": "timeout"}))
        collector.admit(dict(coordinates))
        self.assertEqual(self.post(url + "/benchmark/next", b"{}"), (200, {"status": "permit", "coordinates": coordinates}))
        self.assertEqual((collector.runs, collector.state), ({}, "admitted"))

    def test_N05_server_shutdown_wakes_a_waiting_request_and_leaves_no_thread(self):
        import threading
        collector, server, thread, url = self.admitted_server(wait=5)
        result = {}
        request = threading.Thread(target=lambda: result.setdefault("value", self.post(url + "/benchmark/next", b"{}")))
        request.start()
        for _ in range(400):
            if collector.waiter:
                break
            import time
            time.sleep(0.005)
        collector.shutdown()
        request.join(3)
        server.shutdown()
        thread.join(3)
        self.assertFalse(request.is_alive() or thread.is_alive())
        self.assertEqual(result["value"], (200, {"status": "none", "reason": "cancelled"}))

    def test_DEF0023_waiting_http_request_gets_the_entry_guard_rejection_when_a_direct_start_wins(self):
        import threading
        collector, server, thread, url = self.admitted_server(wait=5)
        self.addCleanup(server.shutdown)
        result = {}
        request = threading.Thread(target=lambda: result.setdefault("value", self.post(url + "/benchmark/next", b"{}")))
        request.start()
        for _ in range(2000):
            with collector.lock:
                if collector.waiter:
                    break
            request.join(0.001)
        self.assertTrue(collector.waiter)
        with collector.lock:
            collector.admit({"kind": "cold", "engine": "memory", "operation": "create", "n": 0, "run": 1, "worker_id": "cold:1"})
            collector.start_cold("memory", "create", 0, 1, "cold:1")
        request.join(3)
        self.assertFalse(request.is_alive())
        self.assertEqual(result["value"], (409, None))
        self.assertFalse(collector.waiter)
        self.assertEqual(self.post(url + "/benchmark/next", b"{}")[0], 409)

    def test_oversized_boundary_has_bounded_consumption_before_rejection(self):
        import io
        handler = object.__new__(self.server.RequestHandlerClass)
        handler.headers = {"Content-Length": str(262145)}
        handler.rfile = io.BytesIO(b"x" * 262145)
        replies = []
        handler.reply = lambda status, value: replies.append((status, value))
        handler.do_POST()
        self.assertEqual(replies, [(413, {"ok": False, "error": "LimitExceeded"})])
        self.assertEqual(handler.rfile.tell(), 262145)


if __name__ == "__main__":
    unittest.main()
