#!/usr/bin/env python3
"""TC-0037 browser experiment controller. Stdlib only; no database/SDK implementation."""
import copy
import hashlib
import json
from pathlib import Path
import re
import threading
import time
from run import ProofCapture, RunnerError, canonical, require
from summary import summarize

RETIREMENT = "browser Worker.terminate() request; not OS process reaping"
CONTINUITY = "thermal/cache/environment continuity across independent workers is uncontrolled"


class BrowserCollector:
    """Record one experiment, with quiet admission and independent literal proof."""
    def __init__(self, output, metadata, parent, source_hash, artifact_hash, *, allow_measurements=False, recheck=None, on_held=None, run_status="contaminated", next_wait_seconds=25):
        self.output = Path(output)
        self.metadata = copy.deepcopy(metadata)
        self.allow_measurements = allow_measurements
        # Only a caller-attested quiet window may produce summaries; contaminated is the default.
        require(run_status in ("ok", "contaminated"), "invalid run status")
        self.run_status = run_status
        require(metadata.get("status") == "reviewed_for_implementation" and metadata.get("measurements") == "not_started", "unreviewed browser workload")
        require(metadata.get("format") == 1 and metadata.get("classes") == ["browser-sdk/memory", "browser-sdk/opfs-protected"], "invalid SDK classes")
        parent_bytes = Path(__file__).with_name("workloads.json").read_bytes()
        require(json.loads(parent_bytes) == parent and metadata.get("source_workload", {}).get("sha256") == hashlib.sha256(parent_bytes).hexdigest(), "parent source digest mismatch")
        require(metadata.get("measurements") == "not_started", "invalid workload state")
        for field in ("schema", "dataset", "cohorts", "sampling", "operation_details"):
            require(metadata[field] == parent[field], "browser/parent workload mismatch")
        expected_cold = []
        for item in parent['cold']:
            transformed = {key: copy.deepcopy(value) for key, value in item.items() if key != 'engines'}
            transformed['classes'] = ['browser-sdk/' + ('opfs-protected' if engine == 'native-protected' else engine) for engine in item['engines']]
            expected_cold.append(transformed)
        require(metadata.get('cold') == expected_cold, 'browser/parent cold workload mismatch')
        require(len(metadata["cohorts"]) == 51 and metadata["sampling"] == {
            "cold_processes": 10, "independent_runs": 5, "samples": 30, "warmups": 5}, "invalid finite matrix")
        for value in (source_hash, artifact_hash):
            require(isinstance(value, str) and re.fullmatch(r"[a-f0-9]{64}", value), "invalid manifest hash")
        self.source_hash = source_hash
        self.artifact_hash = artifact_hash
        self.workload_hash = hashlib.sha256(canonical(metadata).encode()).hexdigest()
        self.runs = {}
        self.workers = set()
        self.documents = {}
        # Between-worker admission (benchmarks/admission-plan.md): one explicit
        # permit per worker, issued only by the parent controller, never the page.
        self.recheck = recheck
        self.on_held = on_held
        self.lock = threading.RLock()
        self.state = "idle"
        self.permit = None
        self.active = None
        self.blocked = []
        self.held = None
        self.hold_started = None
        self.contaminated = set()
        # Page permit transport (admission-plan reviewed amendment): one waiter, one claim.
        require(isinstance(next_wait_seconds, (int, float)) and 0 < next_wait_seconds <= 25, "invalid permit wait bound")
        self.next_wait_seconds = next_wait_seconds
        self.condition = threading.Condition(self.lock)
        self.waiter = None
        self.starts = 0
        self.claimed = False
        self.closing = False
        self.before_admit = None

    def log(self, event, **fields):
        record = {"event": event, "monotonic_ns": time.monotonic_ns(), "state": self.state, **fields}
        self.output.mkdir(parents=True, exist_ok=True)
        with (self.output / "control.jsonl").open("a", encoding="utf-8") as stream:
            stream.write(canonical(record) + "\n")
        return record

    def reject(self, reason, block=True):
        with self.lock:
            if block and self.state != "cancelled":
                self.blocked.append({"run_id": None, "reason": reason})
                self.condition.notify_all()
            self.log("rejected", reason=reason)
        raise RunnerError(reason)

    def coordinates(self, value):
        require(isinstance(value, dict), "invalid admission coordinates")
        if value.get("kind") == "warm":
            require(set(value) == {"kind", "engine", "cohort", "run", "worker_id"}, "invalid warm admission keys")
            self.warm_plan(value["engine"], value["cohort"], value["run"], value["worker_id"])
        elif value.get("kind") == "resources":
            require(set(value) == {"kind", "engine", "run", "worker_id"}, "invalid resource admission keys")
            self.resource_plan(value["engine"], value["run"], value["worker_id"])
        else:
            require(value.get("kind") == "cold" and set(value) == {"kind", "engine", "operation", "n", "run", "worker_id"}, "invalid admission kind/keys")
            self.cold_plan(value["engine"], value["operation"], value["n"], value["run"], value["worker_id"])
        return canonical(value)

    def admit(self, coordinates, environment=None):
        with self.lock:
            require(self.allow_measurements, "quiet approval required")
            if self.state == "cancelled":
                self.reject("controller cancelled", block=False)
            if self.blocked:
                self.reject("admission blocked pending explicit recovery")
            if self.state not in ("idle", "held", "recovered"):
                self.reject("pre-issued permit: worker or permit already active")
            try:
                permit = self.coordinates(coordinates)
            except RunnerError as error:
                self.reject("invalid or stale admission coordinates: " + str(error))
            if not callable(self.recheck) or tuple(self.recheck()) != (self.source_hash, self.artifact_hash):
                self.reject("frozen source/artifact drift before admission")
            self.permit = permit
            self.claimed = False
            self.before_admit = self.state
            self.state = "admitted"
            self.condition.notify_all()
            record = self.log("admit", coordinates=copy.deepcopy(coordinates), environment=copy.deepcopy(environment),
                              hold_started_monotonic_ns=self.hold_started, continuity=CONTINUITY)
            return {"coordinates": copy.deepcopy(coordinates), "admitted_monotonic_ns": record["monotonic_ns"]}

    def consume(self, coordinates, plan):
        """Consume the exact one-use permit only after complete start preflight (DEF-0029).

        Order: non-mutating permit match, drift recheck (clears and blocks), non-mutating
        plan, then consumption and start-generation commit. Returns the planned run id.
        """
        require(self.allow_measurements, "quiet approval required")
        if self.state != "admitted" or self.blocked or self.permit != canonical(coordinates):
            self.reject("missing, blocked or mismatched one-use permit", block=False)
        # Every start route rechecks the frozen closure at consumption.
        if not callable(self.recheck) or tuple(self.recheck()) != (self.source_hash, self.artifact_hash):
            self.permit = None
            self.claimed = False
            self.state = self.before_admit
            self.reject("frozen source/artifact drift at permit consumption")
        try:
            run_id = plan()
        except RunnerError as error:
            # A plan failure leaves the admitted permit unconsumed for explicit REVOKE/RECOVER.
            self.reject("start preflight rejected: " + str(error), block=False)
        self.permit = None
        self.claimed = False
        self.state = "active"
        # DEF-0023: any waiter registered before this start must retire, even if
        # the worker also completes before that waiter reacquires the lock.
        self.starts += 1
        self.condition.notify_all()
        return run_id

    def declare(self, declaration):
        with self.lock:
            require(self.allow_measurements, "quiet approval required")
            require(isinstance(declaration, dict) and len(canonical(declaration)) <= 4096, "invalid competing-work declaration")
            if self.state != "held":
                if self.active is not None:
                    self.contaminated.add(self.active)
                self.reject("competing work declared outside held state")
            self.log("declare", declaration=copy.deepcopy(declaration))

    def recover(self, run_id, reason):
        with self.lock:
            require(self.state != "cancelled" and self.blocked and isinstance(reason, str) and reason, "nothing to recover")
            failed = {entry["run_id"] for entry in self.blocked if entry["run_id"] is not None}
            require(failed <= {run_id} and (run_id is None or run_id in failed), "recovery must name the failed run")
            self.log("recovered", run_id=run_id, reason=reason, cleared=self.blocked)
            self.blocked = []
            if self.state == "failed":
                self.state = "recovered"

    def cancel(self, reason):
        with self.lock:
            require(self.state != "cancelled" and isinstance(reason, str) and reason, "already cancelled")
            if self.active is not None:
                self.contaminated.add(self.active)
            self.permit = None
            self.log("cancelled", reason=reason, active_run=self.active)
            self.state = "cancelled"
            self.condition.notify_all()

    def next_permit(self):
        """Transport an already parent-issued permit; never admits, starts or consumes."""
        none = lambda reason: {"status": "none", "reason": reason}
        with self.condition:
            require(self.allow_measurements, "quiet approval required")
            if self.state == "cancelled" or self.closing:
                return none("cancelled")
            if self.active is not None:
                self.reject("next permit requested during an active worker", block=False)
            if self.blocked:
                return none("blocked")
            require(self.state in ("idle", "held", "recovered", "admitted"), "invalid next-permit state")
            if self.waiter or self.claimed:
                return none("already_claimed")
            token, starts = object(), self.starts
            self.waiter = token
            try:
                # Bounded wait on the page main thread, outside every worker and sample clock.
                deadline = time.monotonic() + self.next_wait_seconds
                while (self.permit is None and self.starts == starts and self.state != "cancelled"
                       and not self.closing and not self.blocked):
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        break
                    self.condition.wait(remaining)
            finally:
                if self.waiter is token:
                    self.waiter = None
            if self.starts != starts:
                self.reject("next permit requested during an active worker", block=False)
            if self.state == "cancelled" or self.closing:
                return none("cancelled")
            if self.blocked:
                return none("blocked")
            if self.permit is None:
                return none("timeout")
            self.claimed = True
            coordinates = json.loads(self.permit)
            self.log("claim", coordinates=coordinates)
            return {"status": "permit", "coordinates": coordinates}

    def revoke(self, reason):
        with self.lock:
            require(self.allow_measurements and self.state == "admitted" and self.active is None and self.permit is not None
                    and isinstance(reason, str) and reason, "no unconsumed permit to revoke")
            self.log("revoked", reason=reason, coordinates=json.loads(self.permit), claimed=self.claimed)
            self.permit = None
            self.claimed = False
            self.state = self.before_admit
            self.blocked.append({"run_id": None, "reason": "permit revoked: " + reason})
            self.condition.notify_all()

    def shutdown(self):
        with self.lock:
            self.closing = True
            self.condition.notify_all()

    def between_workers(self):
        with self.lock:
            return copy.deepcopy(self.held) if self.state == "held" else None

    def warm_plan(self, engine, cohort, run, worker_id):
        require(engine in ("memory", "opfs-protected"), "invalid engine class")
        require(type(cohort) is int and 0 <= cohort < 51 and type(run) is int and 1 <= run <= 5, "invalid run coordinates")
        require(isinstance(worker_id, str) and re.fullmatch(r"[A-Za-z0-9:_-]{1,100}", worker_id), "invalid worker identity")
        run_id = f"{engine}.c{cohort:02}.r{run}"
        require(run_id not in self.runs and worker_id not in self.workers, "duplicate run/worker identity")
        require(not (self.output / (run_id + ".json")).exists(), "existing run evidence")
        return run_id

    def start(self, engine, cohort, run, worker_id):
        with self.lock:
            run_id = self.consume({"kind": "warm", "engine": engine, "cohort": cohort, "run": run, "worker_id": worker_id},
                                  lambda: self.warm_plan(engine, cohort, run, worker_id))
            self.active = run_id
            self.log("start", run_id=run_id)
            return self.started_warm(run_id, engine, cohort, run, worker_id)

    def started_warm(self, run_id, engine, cohort, run, worker_id):
        parameters = copy.deepcopy(self.metadata["cohorts"][cohort])
        identity = {"class": "browser-sdk/" + engine, "profile": "release", "unit": "ns",
                    "endpoint": "sdk-invocation-to-last-callback" if parameters["operation"] == "reactivity" else "sdk-invocation-to-decoded-return",
                    "artifact_sha256": self.artifact_hash, "source_sha256": self.source_hash,
                    "workload_sha256": self.workload_hash, "parameters": parameters}
        self.runs[run_id] = {"identity": copy.deepcopy(identity), "run": run, "engine": engine,
                             "cohort": cohort, "worker_id": worker_id, "capture": ProofCapture(parameters["N"]),
                             "started_monotonic_ns": time.monotonic_ns(), "finished": False}
        self.workers.add(worker_id)
        return {"run_id": run_id, "identity": copy.deepcopy(identity), "cohort": parameters,
                "measurements_allowed": True, "worker_id": worker_id,
                "owned_names": self.owned_names(self.runs[run_id])}

    def cold_plan(self, engine, operation, n, run, worker_id):
        require(engine in ('memory', 'opfs-protected') and type(n) is int and type(run) is int and 1 <= run <= 10, 'invalid cold coordinates')
        declared = any(item.get('operation') == operation
                       and 'browser-sdk/' + engine in item.get('classes', [])
                       and (n in item['N'] if isinstance(item['N'], list) else n == item['N'])
                       for item in self.metadata['cold'])
        require(declared, 'undeclared cold class')
        require(isinstance(worker_id, str) and re.fullmatch(r'[A-Za-z0-9:_-]{1,100}', worker_id), 'invalid worker identity')
        run_id = f'cold.{engine}.{operation}.n{n}.r{run}'
        require(run_id not in self.runs and worker_id not in self.workers, 'duplicate cold run/worker')
        require(not (self.output / (run_id + '.json')).exists(), 'existing cold evidence')
        return run_id

    def start_cold(self, engine, operation, n, run, worker_id):
        with self.lock:
            run_id = self.consume({"kind": "cold", "engine": engine, "operation": operation, "n": n, "run": run, "worker_id": worker_id},
                                  lambda: self.cold_plan(engine, operation, n, run, worker_id))
            self.active = run_id
            self.log("start", run_id=run_id)
            return self.started_cold(run_id, engine, operation, n, run, worker_id)

    def started_cold(self, run_id, engine, operation, n, run, worker_id):
        parameters = {'operation': operation, 'N': n, 'Q': 0, 'observers': 0}
        identity = {'class': 'browser-sdk/' + engine, 'profile': 'release', 'unit': 'ns',
                    'endpoint': 'sdk-open-invocation-to-decoded-ready',
                    'artifact_sha256': self.artifact_hash, 'source_sha256': self.source_hash,
                    'workload_sha256': self.workload_hash, 'parameters': parameters}
        self.runs[run_id] = {'kind': 'cold', 'identity': copy.deepcopy(identity), 'run': run,
                            'engine': engine, 'cohort': parameters, 'worker_id': worker_id,
                            'capture': ProofCapture(n), 'started_monotonic_ns': time.monotonic_ns(), 'finished': False}
        self.workers.add(worker_id)
        return {'run_id': run_id, 'identity': copy.deepcopy(identity), 'cohort': parameters,
                'measurements_allowed': True, 'worker_id': worker_id,
                'owned_names': self.owned_names(self.runs[run_id])}

    def resource_plan(self, engine, run, worker_id):
        require(engine in ("memory", "opfs-protected") and type(run) is int and run == 1, "invalid resource coordinates")
        require(isinstance(worker_id, str) and re.fullmatch(r"[A-Za-z0-9:_-]{1,100}", worker_id), "invalid worker identity")
        run_id = f"resources.{engine}.r{run}"
        require(run_id not in self.runs and worker_id not in self.workers, "duplicate resource run/worker")
        require(not (self.output / (run_id + ".json")).exists(), "existing resource evidence")
        return run_id

    def start_resources(self, engine, run, worker_id):
        with self.lock:
            run_id = self.consume({"kind": "resources", "engine": engine, "run": run, "worker_id": worker_id},
                                  lambda: self.resource_plan(engine, run, worker_id))
            self.active = run_id
            self.log("start", run_id=run_id)
            # Declared store semantics (browser-plan resource classes); never native thirty-fresh-store.
            parameters = {"operation": "retained-resources", "N": 100, "Q": 0, "observers": 1, "mode": "membership", "sharing": "equivalent",
                          "store": "fresh-memory-per-cycle" if engine == "memory" else "same-owned-store-reopen"}
            identity = {"class": "browser-sdk/" + engine, "profile": "release", "unit": "bytes",
                        "endpoint": "untimed-open-observe-close-resources",
                        "resource_artifact": "harness worker importing unchanged ordinary WASM and production worker host",
                        "artifact_sha256": self.artifact_hash, "source_sha256": self.source_hash,
                        "workload_sha256": self.workload_hash, "parameters": parameters}
            self.runs[run_id] = {"kind": "resources", "identity": copy.deepcopy(identity), "run": run, "engine": engine,
                                 "cohort": parameters, "worker_id": worker_id, "capture": ProofCapture(100), "checkpoints": [],
                                 "started_monotonic_ns": time.monotonic_ns(), "finished": False}
            self.workers.add(worker_id)
            return {"run_id": run_id, "identity": copy.deepcopy(identity), "cohort": copy.deepcopy(parameters),
                    "measurements_allowed": True, "worker_id": worker_id, "owned_names": self.owned_names(self.runs[run_id])}

    @staticmethod
    def checked_resource(record):
        keys = {"phase", "cycle", "ordinary_worker_linear_bytes", "harness_worker_linear_bytes", "js_heap_bytes", "ua_specific"}
        require(isinstance(record, dict) and set(record) == keys, "invalid resource checkpoint keys")
        # Unavailable metrics stay unknown; ordinary-worker allocation is never inferred.
        require(record["ordinary_worker_linear_bytes"] is None and record["js_heap_bytes"] is None, "fabricated unavailable metric")
        harness = record["harness_worker_linear_bytes"]
        require(harness is None or (type(harness) is int and 0 <= harness and harness % 65536 == 0), "invalid harness linear memory")
        ua = record["ua_specific"]
        require(isinstance(ua, dict), "invalid profiler record")
        if ua.get("status") == "observed":
            value = ua.get("value")
            require(set(ua) == {"status", "value"} and isinstance(value, dict) and type(value.get("bytes")) is int and value["bytes"] >= 0
                    and isinstance(value.get("breakdown"), list), "invalid profiler observation")
        else:
            require(set(ua) in ({"status", "reason"}, {"status", "reason", "serialized_bytes"}) and ua["status"] == "unknown" and isinstance(ua["reason"], str)
                    and (type(ua.get("serialized_bytes", 0)) is int), "invalid unknown profiler state")

    def checkpoint(self, run_id, record):
        with self.lock:
            state = self.owned(run_id)
            require(state.get("kind") == "resources", "checkpoint outside resource run")
            self.checked_resource(record)
            capture, index = state["capture"], len(state["checkpoints"])
            expected = ("baseline", None) if index == 0 else ("after_open" if index % 2 else "after_close", (index - 1) // 2)
            require(index < 61 and (record["phase"], record["cycle"]) == expected and type(record["cycle"]) in (int, type(None)), "invalid resource checkpoint sequence")
            # First/final independent proofs precede the cycle0/cycle29 after_open checkpoints.
            require(capture.phase is None and ("initial" in capture.proof) == (index >= 1) and ("final" in capture.proof) == (index >= 59), "resource proof at wrong cycle")
            self.output.mkdir(parents=True, exist_ok=True)
            with (self.output / (run_id + ".checkpoints.jsonl")).open("a", encoding="utf-8") as stream:
                stream.write(canonical(record) + "\n")
            state["checkpoints"].append(copy.deepcopy(record))
            return {"reply": "CONTINUE"}

    @staticmethod
    def owned_names(state):
        if state["engine"] == "memory":
            return []
        nonce = hashlib.sha256(state["worker_id"].encode()).hexdigest()[:12]
        if state.get('kind') == 'resources':
            stem = f"phase-seven-sdk-owned-bench-resources-r{state['run']}-{nonce}"
        elif state.get('kind') == 'cold':
            parameters = state['cohort']
            stem = f"phase-seven-sdk-owned-bench-cold-{parameters['operation']}-n{parameters['N']}-r{state['run']}-{nonce}"
        else:
            stem = f"phase-seven-sdk-owned-bench-opfs-protected-c{state['cohort']:02}-r{state['run']}-{nonce}"
        return [stem + ".sqlite", stem + ".neighbor.sqlite"]

    def validate_cleanup(self, state, lifecycle):
        keys = {"owner_closed", "worker_terminated", "removed_names", "pool_before", "pool_after", "canary_before_sha256", "canary_after_sha256"}
        require(isinstance(lifecycle, dict) and set(lifecycle) == keys, "missing cleanup evidence")
        require(lifecycle["owner_closed"] is True and lifecycle["worker_terminated"] is True, "live owner/worker at acceptance")
        require(lifecycle["removed_names"] == self.owned_names(state), "owned cleanup identity mismatch")
        if state["engine"] == "memory":
            require(all(lifecycle[key] is None for key in ("pool_before", "pool_after", "canary_before_sha256", "canary_after_sha256")), "memory cleanup claims storage access")
        else:
            before = lifecycle["pool_before"]
            require(isinstance(before, list) and len(before) == 2 and all(type(value) is int for value in before) and before[0] == 24 and 0 <= before[1] <= before[0], "invalid pool observation")
            require(lifecycle["pool_after"] == before, "pool inventory changed")
            digest = lifecycle["canary_before_sha256"]
            require(isinstance(digest, str) and re.fullmatch(r"[a-f0-9]{64}", digest) and lifecycle["canary_after_sha256"] == digest, "canary bytes changed")

    def owned(self, run_id):
        require(run_id in self.runs and not self.runs[run_id]["finished"], "unknown or finished run")
        return self.runs[run_id]

    def feed(self, run_id, body):
        with self.lock:
            state = self.owned(run_id)
            require(isinstance(body, bytes) and 0 < len(body) <= 262144, "invalid bounded proof transfer")
            for line in body.splitlines(keepends=True):
                state["capture"].feed(line)
                if line == b"READY\n":
                    state["ready_receipt_monotonic_ns"] = time.monotonic_ns()

    def sample(self, run_id, sample):
        with self.lock:
            self.owned(run_id)
            require(isinstance(sample, dict) and sample.get("phase") in ("warmup", "measured"), "invalid raw sample")
            require(type(sample.get("index")) is int and sample["index"] >= 0, "invalid raw index")
            self.output.mkdir(parents=True, exist_ok=True)
            with (self.output / (run_id + ".samples.jsonl")).open("a", encoding="utf-8") as stream:
                stream.write(canonical(sample) + "\n")

    def finish(self, run_id, document):
        with self.lock:
            state = self.owned(run_id)
            self.output.mkdir(parents=True, exist_ok=True)
            # Preserve raw samples/report even when identity/proof/acceptance later fails.
            raw = (canonical(document) + "\n").encode()
            (self.output / (run_id + ".raw.json")).write_bytes(raw)
            state["finished"] = True
            self.active = None
            try:
                require(self.state == "active" and run_id not in self.contaminated, "cancelled or contaminated active run")
                result, saved = self.accept(state, run_id, document)
            except (RunnerError, ValueError, KeyError, TypeError, OSError) as error:
                self.held = None
                if self.state != "cancelled":
                    self.state = "failed"
                    self.blocked.append({"run_id": run_id, "reason": str(error)})
                self.log("run_failed", run_id=run_id, reason=str(error), raw_sha256=hashlib.sha256(raw).hexdigest())
                raise
            # Held only after the exact raw/accepted reports are persisted and the
            # owner/worker/pool/canary cleanup evidence has been validated above.
            self.held = {"run_id": run_id, "worker_id": state["worker_id"], "raw_sha256": hashlib.sha256(raw).hexdigest(),
                         "accepted_sha256": hashlib.sha256(saved).hexdigest(), "summary_ready": result["summary_ready"],
                         "worker_retirement": RETIREMENT, "continuity": CONTINUITY}
            self.state = "held"
            self.hold_started = self.log("between_workers", **self.held)["monotonic_ns"]
            if self.on_held is not None:
                self.on_held(copy.deepcopy(self.held))
            return result

    def accept(self, state, run_id, document):
        self.validate_cleanup(state, document.get("lifecycle"))
        if state.get("kind") == "resources":
            return self.accept_resources(state, run_id, document)
        require(document.get("format") == 1 and document.get("identity") == state["identity"], "mixed artifact/workload identity")
        cold = state.get('kind') == 'cold'
        protocol = {'runs': 10, 'warmups': 0, 'samples': 1} if cold else {'runs': 5, 'warmups': 5, 'samples': 30}
        require(document.get("protocol") == protocol, "wrong sampling protocol")
        require(isinstance(document.get("runs"), list) and len(document["runs"]) == 1, "invalid per-run document")
        result = document["runs"][0]
        require(result.get("run") == state["run"] and result.get("status") == "ok", "failed run")
        samples = result.get("samples")
        require(isinstance(samples, list) and len(samples) == protocol['warmups'] + protocol['samples'], "incomplete sample count")
        for phase, count in (("warmup", protocol['warmups']), ("measured", protocol['samples'])):
            selected = [sample for sample in samples if sample.get("phase") == phase]
            require(len(selected) == count and {sample.get("index") for sample in selected} == set(range(count)), "sample index mismatch")
        for sample in samples:
            require(type(sample.get("index")) is int and sample["index"] >= 0, "noninteger sample index")
            require(sample.get("status") == "ok" and sample.get("correct") is True and type(sample.get("value")) is int and 0 <= sample["value"] <= 9007199254740991, "failed/invalid sample")
            if cold:
                outer = sample.get('worker_creation_to_sdk_ready_ns')
                require(type(outer) is int and sample['value'] <= outer <= 9007199254740991, 'invalid outer cold endpoint')
        if cold:
            require(state.get('ready_receipt_monotonic_ns') is not None, 'missing cold readiness receipt')
        state["capture"].feed(("REPORT " + canonical(document) + "\n").encode())
        proof = state["capture"].finish()
        saved = {"status": "accepted_run", "run_status": self.run_status, "worker_id": state["worker_id"], "report": document,
                 "proof": proof["proof"], "ready_receipt_monotonic_ns": state.get("ready_receipt_monotonic_ns"),
                 "finished_receipt_monotonic_ns": time.monotonic_ns()}
        saved = (canonical(saved) + "\n").encode()
        (self.output / (run_id + ".json")).write_bytes(saved)
        group = ('cold', state['engine'], state['cohort']['operation'], state['cohort']['N']) if cold else (state["engine"], state["cohort"])
        self.documents.setdefault(group, []).append(copy.deepcopy(document))
        ready = self.run_status == "ok" and len(self.documents[group]) == protocol['runs']
        if ready:
            summary = summarize(self.documents[group])
            filename = f'cold.{group[1]}.{group[2]}.n{group[3]}.summary.json' if cold else f"{group[0]}.c{group[1]:02}.summary.json"
            (self.output / filename).write_text(canonical(summary) + "\n")
        return {"status": "accepted" if self.run_status == "ok" else "accepted_contaminated", "summary_ready": ready}, saved


    def accept_resources(self, state, run_id, document):
        require(document.get("format") == 1 and document.get("identity") == state["identity"], "mixed artifact/workload identity")
        require(document.get("protocol") == {"cycles": 30, "checkpoints": 61}, "wrong resource protocol")
        require(document.get("runs") == [{"run": state["run"], "status": "ok"}], "failed resource run")
        points = state["checkpoints"]
        require(len(points) == 61, "incomplete resource checkpoints")
        state["capture"].feed(("REPORT " + canonical(document) + "\n").encode())
        proof = state["capture"].finish()
        baseline, final = points[0]["harness_worker_linear_bytes"], points[-1]["harness_worker_linear_bytes"]
        metric = {"tool": "harness-worker WebAssembly.Memory.buffer.byteLength via separate MessagePort; measureUserAgentSpecificMemory feature-detected",
                  "cycles": 30, "baseline_harness_worker_linear_bytes": baseline, "final_post_close_harness_worker_linear_bytes": final,
                  "retained_delta_harness_worker_linear_bytes": final - baseline if baseline is not None and final is not None else None,
                  "ordinary_worker_linear_bytes": None, "js_heap_bytes": None,
                  "ua_specific_statuses": sorted({point["ua_specific"]["status"] for point in points}),
                  "interpretation": "allocated harness-worker linear memory and retained delta; not live Rust heap or renderer RSS; not an allocator leak conclusion"}
        saved = {"status": "accepted_run", "run_status": self.run_status, "worker_id": state["worker_id"], "report": document,
                 "proof": proof["proof"], "checkpoints": points, "metric": metric, "finished_receipt_monotonic_ns": time.monotonic_ns()}
        saved = (canonical(saved) + "\n").encode()
        (self.output / (run_id + ".json")).write_bytes(saved)
        return {"status": "accepted" if self.run_status == "ok" else "accepted_contaminated", "summary_ready": False}, saved


def create_server(root, collector, port=0):
    """Loopback fixture only; ordinary SDK/WASM files remain unmodified."""
    from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
    from urllib.parse import urlsplit, parse_qs
    from run import strict_json
    root = Path(root).resolve(strict=True)

    class Handler(SimpleHTTPRequestHandler):
        def __init__(self, *args, **kwargs):
            super().__init__(*args, directory=str(root), **kwargs)

        def log_message(self, *_):
            pass  # Never log proof payloads, URLs, keys or application data.

        def end_headers(self):
            self.send_header("Cross-Origin-Opener-Policy", "same-origin")
            self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
            self.send_header("Cache-Control", "no-store")
            super().end_headers()

        def reply(self, status, value):
            body = canonical(value).encode()
            self.send_response(status)
            self.send_header("Content-Type", "application/json; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)

        def do_GET(self):
            path = urlsplit(self.path).path
            if path.startswith("/benchmark/") and path != "/benchmark/meta":
                return self.reply(405, {"ok": False, "error": "InvalidBenchmarkRequest"})
            if path == "/benchmark/meta":
                return self.reply(200, {"metadata": collector.metadata,
                                        "measurements_allowed": collector.allow_measurements})
            return super().do_GET()

        def do_POST(self):
            try:
                length = int(self.headers.get("Content-Length", "-1"))
                if not 0 <= length <= 262144:
                    if length > 262144:
                        # Drain only the bounded cap+1 sentinel before closing.
                        # Otherwise a client writing the one-byte-over boundary
                        # can observe a reset instead of the declared 413 reply.
                        self.rfile.read(min(length, 262145))
                    return self.reply(413, {"ok": False, "error": "LimitExceeded"})
                origin = self.headers.get("Origin")
                require(origin is None or origin == "http://127.0.0.1:" + str(self.server.server_port), "foreign origin")
                raw = self.rfile.read(length)
                require(len(raw) == length, "incomplete transfer")
                url = urlsplit(self.path)
                query = parse_qs(url.query, strict_parsing=True)
                if url.path == "/benchmark/next":
                    # Exact empty request; transports an existing permit only.
                    require(url.query == "" and raw == b"{}", "invalid next-permit request")
                    value = collector.next_permit()
                elif url.path in ("/benchmark/start", '/benchmark/start-cold', '/benchmark/start-resources'):
                    args = strict_json(raw.decode("utf-8"))
                    if url.path == '/benchmark/start-resources':
                        require(set(args) == {'engine', 'run', 'worker_id'}, 'invalid resource start keys')
                        value = collector.start_resources(**args)
                    elif url.path == '/benchmark/start-cold':
                        require(set(args) == {'engine', 'operation', 'n', 'run', 'worker_id'}, 'invalid cold start keys')
                        value = collector.start_cold(**args)
                    else:
                        require(set(args) == {"engine", "cohort", "run", "worker_id"}, "invalid start keys")
                        value = collector.start(**args)
                else:
                    require(set(query) == {"run"} and len(query["run"]) == 1, "invalid run query")
                    run = query["run"][0]
                    if url.path == "/benchmark/lines":
                        collector.feed(run, raw)
                        value = {"ok": True}
                    elif url.path == "/benchmark/sample":
                        collector.sample(run, strict_json(raw.decode("utf-8")))
                        value = {"ok": True}
                    elif url.path == "/benchmark/checkpoint":
                        value = collector.checkpoint(run, strict_json(raw.decode("utf-8")))
                    elif url.path == "/benchmark/finish":
                        value = collector.finish(run, strict_json(raw.decode("utf-8")))
                    else:
                        raise RunnerError("unknown benchmark route")
                return self.reply(200, value)
            except (RunnerError, ValueError, KeyError, TypeError, UnicodeError):
                return self.reply(409, {"ok": False, "error": "InvalidBenchmarkRequest"})

    return ThreadingHTTPServer(("127.0.0.1", port), Handler)
