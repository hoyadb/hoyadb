"""TC-0048 QUALITY01/02: reject misleading coverage and mutation evidence."""

import copy
import json
import tempfile
import unittest
from pathlib import Path
import subprocess
import sys

import quality


def measurement_fixture():
    return {"comparison": {"kind": "coverage", "target": "synthetic-host", "profile": "dev",
                           "compiler": "rustc synthetic / LLVM synthetic", "tool": "cargo-llvm-cov 0.9.1",
                           "command": ["cargo", "llvm-cov", "--workspace"], "environment": {}}}


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        self.addCleanup(self.scratch.cleanup)
        self.root = Path(self.scratch.name)
        self.members = ["crates/" + name for name in quality.COMPONENTS]
        (self.root / "Cargo.toml").write_text("[workspace]\nmembers = " + json.dumps(self.members))
        self.files = []
        for name in quality.COMPONENTS:
            path = self.root / "crates" / name / "src/lib.rs"
            path.parent.mkdir(parents=True)
            path.write_text("pub fn answer() -> bool { true }\n")
            self.files.append({"filename": str(path), "summary": {
                metric: {"count": 4, "covered": 3, "percent": 75}
                for metric in ("lines", "regions", "functions")
            }, "segments": [[1, 1, 0, True, True, False]]})
        self.snapshot = quality.source_snapshot(self.root)
        self.report = {"type": "llvm.coverage.json.export", "version": "2.0.1",
                       "data": [{"files": self.files}]}

    def test_component_counts_and_missing_branches_are_honest(self):
        result = quality.coverage_summary(self.report, self.root, self.snapshot)
        self.assertEqual(set(result["components"]), set(quality.COMPONENTS))
        for component in result["components"].values():
            self.assertEqual(component["lines"], {"covered": 3, "count": 4})
            self.assertIsNone(component["branches"])
            self.assertEqual(component["uncovered"][0]["region_start_lines"], [1])

    def test_new_workspace_member_cannot_be_silently_unmeasured(self):
        (self.root / "Cargo.toml").write_text(
            "[workspace]\nmembers = " + json.dumps(self.members + ["crates/new-adapter"]))
        with self.assertRaisesRegex(ValueError, "workspace"):
            quality.coverage_summary(self.report, self.root, quality.source_snapshot(self.root))

    def test_changed_or_added_source_invalidates_measurement(self):
        path = self.root / "crates" / quality.COMPONENTS[0] / "src/new.rs"
        path.write_text("pub fn new() {}")
        with self.assertRaisesRegex(ValueError, "source changed"):
            quality.coverage_summary(self.report, self.root, self.snapshot)

    def test_missing_component_duplicate_file_and_impossible_counts_reject(self):
        variants = []
        missing = copy.deepcopy(self.report)
        missing["data"][0]["files"].pop()
        variants.append(missing)
        duplicate = copy.deepcopy(self.report)
        duplicate["data"][0]["files"].append(duplicate["data"][0]["files"][0])
        variants.append(duplicate)
        for count in (-1, True, 1.5):
            invalid = copy.deepcopy(self.report)
            invalid["data"][0]["files"][0]["summary"]["lines"]["count"] = count
            variants.append(invalid)
        invalid = copy.deepcopy(self.report)
        invalid["data"][0]["files"][0]["summary"]["lines"]["covered"] = 5
        variants.append(invalid)
        for report in variants:
            with self.subTest(report=report), self.assertRaises(ValueError):
                quality.coverage_summary(report, self.root, self.snapshot)

    def test_noninstrumented_sources_remain_visible(self):
        path = self.root / "crates" / quality.COMPONENTS[0] / "src/extra.rs"
        path.write_text("pub fn unused() {}")
        snapshot = quality.source_snapshot(self.root)
        result = quality.coverage_summary(self.report, self.root, snapshot)
        self.assertIn(str(path.relative_to(self.root)), result["uninstrumented_sources"])

    def test_excluded_bridges_and_build_outputs_do_not_change_workspace_evidence(self):
        for relative in (
            "crates/hoyadb-native/target/debug/build/dependency/out/bindgen.rs",
            "crates/hoyadb-web/src/lib.rs",
            "crates/hoyadb-web/Cargo.toml",
            "crates/hoyadb-core/target/debug/build/dependency/out/generated.rs",
            "target/debug/build/dependency/out/generated.rs",
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("generated or separately measured input\n")
        self.assertEqual(quality.source_snapshot(self.root), self.snapshot)
        result = quality.coverage_summary(self.report, self.root, self.snapshot)
        self.assertEqual(result["uninstrumented_sources"], [])

    def test_workspace_build_and_test_inputs_still_invalidate_evidence(self):
        for relative in (
            "crates/hoyadb-core/build.rs",
            "crates/hoyadb-core/Cargo.toml",
            "crates/hoyadb-core/tests/new.rs",
            "crates/hoyadb-core/examples/new.rs",
            "crates/hoyadb-core/benches/new.rs",
            "tests/conformance/new.rs",
        ):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            with self.subTest(relative=relative):
                path.write_text("new input\n")
                with self.assertRaisesRegex(ValueError, "source changed"):
                    quality.coverage_summary(self.report, self.root, self.snapshot)
                snapshot = quality.source_snapshot(self.root)
                path.write_text("changed input\n")
                with self.assertRaisesRegex(ValueError, "source changed"):
                    quality.coverage_summary(self.report, self.root, snapshot)
                path.unlink()

    def test_missing_instrumentation_is_not_complete_coverage(self):
        self.report["data"][0]["files"][0]["summary"]["lines"] = {
            "count": 0, "covered": 0, "percent": 0}
        with self.assertRaises(ValueError):
            quality.coverage_summary(self.report, self.root, self.snapshot)

    def test_memory_reexport_reports_implementation_owner_without_fake_percentage(self):
        name = "hoyadb-storage-memory"
        path = self.root / "crates" / name / "src/lib.rs"
        path.write_text("//! Memory alias.\npub use hoyadb_core::storage::SnapshotStorage as MemoryStorage;\n")
        self.report["data"][0]["files"] = [f for f in self.files if f["filename"] != str(path)]
        result = quality.coverage_summary(self.report, self.root, quality.source_snapshot(self.root))
        self.assertEqual(result["components"][name]["implementation_in"], "hoyadb-core")
        self.assertIsNone(result["components"][name]["lines"])

    def test_established_coverage_floor_rejects_regressions_and_lost_instrumentation(self):
        measured = quality.coverage_summary(self.report, self.root, self.snapshot)
        measured["measurement"] = measurement_fixture()
        for branch in (False, True):
            baseline = copy.deepcopy(measured)
            current = copy.deepcopy(measured)
            key = "branches" if branch else "lines"
            baseline["components"][quality.COMPONENTS[0]][key] = {"covered": 3, "count": 4}
            for values in ({"covered": 5, "count": 8}, None):
                current["components"][quality.COMPONENTS[0]][key] = values
                with self.subTest(branch=branch, values=values), self.assertRaises(ValueError):
                    quality.check_floors(current, baseline)
        quality.check_floors(measured, copy.deepcopy(measured))

    def test_unknown_coverage_is_not_a_floor_and_components_cannot_disappear(self):
        measured = quality.coverage_summary(self.report, self.root, self.snapshot)
        measured["measurement"] = measurement_fixture()
        baseline = copy.deepcopy(measured)
        baseline["components"][quality.COMPONENTS[0]]["branches"] = None
        quality.check_floors(measured, baseline)
        del measured["components"][quality.COMPONENTS[0]]
        with self.assertRaises(ValueError):
            quality.check_floors(measured, baseline)

    def mutation_report(self, summaries):
        keys = {"CaughtMutant": "caught", "MissedMutant": "missed",
                "Timeout": "timeout", "Unviable": "unviable", "Success": "success"}
        counts = dict.fromkeys(keys.values(), 0)
        for summary in summaries:
            counts[keys[summary]] += 1
        return {"cargo_mutants_version": "27.1.0", "end_time": "2026-10-29T12:00:00Z",
                "total_mutants": len(summaries), **counts,
                "outcomes": [{"scenario": "Baseline", "summary": "Success"}] + [
                    {"scenario": {"Mutant": {"name": str(i)}}, "summary": summary}
                    for i, summary in enumerate(summaries)]}

    def test_unviable_is_separate_from_caught(self):
        result = quality.mutation_summary(self.mutation_report(["CaughtMutant", "Unviable"]))
        self.assertEqual(result["caught"], 1)
        self.assertEqual(result["unviable"], 1)
        self.assertEqual(result["score"], 100)
        self.assertIn("selected", result["scope"])
        self.assertEqual(result["total_mutants"], 2)

    def test_survivors_timeouts_empty_and_unclassified_runs_fail(self):
        for values in (["MissedMutant"], ["Timeout"], [], ["Success"], ["Unviable"]):
            with self.subTest(values=values), self.assertRaises(ValueError):
                quality.mutation_summary(self.mutation_report(values))

    def test_incomplete_baseline_and_inconsistent_counts_fail(self):
        report = self.mutation_report(["CaughtMutant"])
        variants = []
        for field, value in (("end_time", None), ("caught", 2), ("total_mutants", 2),
                             ("cargo_mutants_version", "other")):
            invalid = copy.deepcopy(report)
            invalid[field] = value
            variants.append(invalid)
        for summary in ("Failure", "Timeout"):
            invalid = copy.deepcopy(report)
            invalid["outcomes"][0]["summary"] = summary
            variants.append(invalid)
        invalid = copy.deepcopy(report)
        invalid["outcomes"].pop(0)
        variants.append(invalid)
        for invalid in variants:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                quality.mutation_summary(invalid)

    def test_partial_selection_cannot_pass_as_completed_mutation_run(self):
        with self.assertRaisesRegex(ValueError, "selected"):
            quality.mutation_summary(self.mutation_report(["CaughtMutant"]), selected_count=2)

    def test_duplicate_or_wrong_mutants_cannot_replace_selected_outcomes(self):
        report = self.mutation_report(["CaughtMutant", "CaughtMutant"])
        report["outcomes"][2]["scenario"] = copy.deepcopy(report["outcomes"][1]["scenario"])
        with self.assertRaises(ValueError):
            quality.mutation_summary(report)
        with self.assertRaises(ValueError):
            quality.mutation_summary(self.mutation_report(["CaughtMutant"]), selected_names={"different"})

    def test_cli_refuses_to_return_success_for_missing_evidence(self):
        result = subprocess.run([sys.executable, quality.__file__, "mutations",
                                 str(self.root / "missing.json")], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("quality evidence rejected", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_cli_requires_snapshot_even_with_consistent_successful_outcomes(self):
        report = self.root / "outcomes.json"
        report.write_text(json.dumps(self.mutation_report(["CaughtMutant"])))
        (self.root / "mutants.json").write_text('[{"name":"0"}]')
        result = subprocess.run([sys.executable, quality.__file__, "mutations", str(report)],
                                capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing snapshot", result.stderr)
        self.assertEqual(result.stdout, "")




class ReviewRegressionTests(unittest.TestCase):
    def setUp(self):
        self.fixture = EvidenceTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        script = self.fixture.root / "tests/tooling/quality.py"
        script.parent.mkdir(parents=True)
        script.write_text(Path(quality.__file__).read_text())
        # Exercise the real CLI entry point with isolated source files. Git is an
        # injected boundary so the Python-only CI image needs no Git executable.
        driver = """import runpy,sys,subprocess
from unittest.mock import patch
status=int(sys.argv.pop(1))
sys.argv=sys.argv[1:]
def git(command, **kwargs):
    assert command[0] == 'git', command
    if status: raise subprocess.CalledProcessError(status, command)
    return 'fixture-revision' if command[1] == 'rev-parse' else ''
with patch('subprocess.check_output', side_effect=git):
    runpy.run_path(sys.argv[0], run_name='__main__')
"""
        self.command = [sys.executable, "-c", driver, "0", str(script)]

    def test_floor_rejects_incomparable_targets_profiles_tools_and_commands(self):
        component = {"lines": {"count": 4, "covered": 3}, "branches": None}
        measurement = measurement_fixture()
        baseline = {"components": {"core": component}, "measurement": measurement}
        for key, value in (("target", "host-b"), ("profile", "release"),
                           ("tool", "tool-2"), ("command", ["cargo", "llvm-cov", "--exclude", "core"])):
            current = copy.deepcopy(baseline)
            current["measurement"]["comparison"][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "not comparable"):
                quality.check_floors(current, baseline)

    def test_capture_records_command_toolchain_profile_target_and_flags(self):
        from unittest.mock import patch
        compiler = "rustc 1.98.1\nhost: synthetic-host\nLLVM version: 22.1.8\n"
        with patch.object(quality.subprocess, "check_output", side_effect=[compiler, "cargo-llvm-cov 0.9.1\n"]):
            record = quality.capture_measurement("coverage", ["cargo", "llvm-cov", "--workspace", "--release"])
        self.assertEqual(record["comparison"]["target"], "synthetic-host")
        self.assertEqual(record["comparison"]["profile"], "release")
        self.assertEqual(record["comparison"]["compiler"], compiler.strip())
        self.assertEqual(record["comparison"]["tool"], "cargo-llvm-cov 0.9.1")
        self.assertEqual(record["command"], ["cargo", "llvm-cov", "--workspace", "--release"])
        self.assertIn("captured_at", record)

    def test_cli_rejection_retains_structured_source_and_tool_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report, snapshot, metadata, output = (root / name for name in
                ("outcomes.json", "source.json", "measurement.json", "summary.json"))
            report.write_text(json.dumps(EvidenceTests().mutation_report(["CaughtMutant", "Timeout"])))
            (root / "mutants.json").write_text('[{"name":"0"},{"name":"1"}]')
            source = quality.source_snapshot(self.fixture.root)
            snapshot.write_text(json.dumps(source))
            metadata.write_text(json.dumps({"comparison": {"kind": "mutations"}, "command": ["cargo", "mutants"]}))
            result = subprocess.run([*self.command, "mutations", str(report),
                "--snapshot", str(snapshot), "--measurement", str(metadata), "--output", str(output),
                "--tool-exit", "3"], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("mutation gate failed", result.stderr)
            saved = json.loads(output.read_text())
            self.assertFalse(saved["gate_passed"])
            self.assertEqual(saved["source_sha256"], source)
            self.assertEqual(saved["raw_counts"]["timeout"], 1)
            self.assertEqual(saved["tool_exit"], 3)
            self.assertEqual(saved["measurement"]["command"], ["cargo", "mutants"])

    def test_cli_mutation_source_drift_is_specific_even_with_valid_outcomes(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            report = root / "outcomes.json"
            report.write_text(json.dumps(EvidenceTests().mutation_report(["CaughtMutant"])))
            (root / "mutants.json").write_text('[{"name":"0"}]')
            (root / "source.json").write_text('{}')
            result = subprocess.run([*self.command, "mutations", str(report),
                "--snapshot", str(root / "source.json")], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("source changed", result.stderr)

    def test_mutation_cli_ignores_bridge_builds_but_rejects_workspace_source_drift(self):
        root = self.fixture.root
        report = root / "outcomes.json"
        report.write_text(json.dumps(self.fixture.mutation_report(["CaughtMutant"])))
        (root / "mutants.json").write_text('[{"name":"0"}]')
        snapshot = root / "source.json"
        snapshot.write_text(json.dumps(quality.source_snapshot(root)))
        generated = root / "crates/hoyadb-native/target/debug/build/dependency/out/bindgen.rs"
        generated.parent.mkdir(parents=True)
        generated.write_text("generated\n")
        args = [*self.command, "mutations", str(report), "--snapshot", str(snapshot)]
        passed = subprocess.run(args, capture_output=True, text=True)
        self.assertEqual(passed.returncode, 0, passed.stderr)
        self.assertTrue(json.loads(passed.stdout)["gate_passed"])
        workspace = root / "crates/hoyadb-core/src/new.rs"
        for change in ("pub fn added() {}\n", "pub fn changed() {}\n", None):
            with self.subTest(change=change):
                snapshot.write_text(json.dumps(quality.source_snapshot(root)))
                if change is None:
                    workspace.unlink()
                else:
                    workspace.write_text(change)
                rejected = subprocess.run(args, capture_output=True, text=True)
                self.assertNotEqual(rejected.returncode, 0)
                self.assertIn("source changed", rejected.stderr)

    def test_coverage_cli_checks_baseline_and_rejects_incomparable_runs(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            source = quality.source_snapshot(self.fixture.root)
            paths = ["crates/" + name + "/src/lib.rs" for name in quality.COMPONENTS]
            files = [{"filename": str(self.fixture.root / path), "summary": {
                key: {"count": 4, "covered": 3} for key in ("lines", "regions", "functions")}}
                for path in paths]
            report = {"type": "llvm.coverage.json.export", "data": [{"files": files}]}
            measured = quality.coverage_summary(report, self.fixture.root, source)
            measurement = measurement_fixture()
            measured["measurement"] = measurement
            for name, value in (("coverage.json", report), ("source.json", source),
                                ("measurement.json", measurement)):
                (root / name).write_text(json.dumps(value))
            args = [*self.command, "coverage", str(root / "coverage.json"),
                    "--snapshot", str(root / "source.json"), "--measurement", str(root / "measurement.json"),
                    "--baseline", str(root / "baseline.json")]
            (root / "baseline.json").write_text(json.dumps(measured))
            passed = subprocess.run(args, capture_output=True, text=True)
            self.assertEqual(passed.returncode, 0, passed.stderr)
            self.assertTrue(json.loads(passed.stdout)["gate_passed"])
            variants = []
            raised = copy.deepcopy(measured)
            raised["components"][quality.COMPONENTS[0]]["lines"] = {"count": 4, "covered": 4}
            variants.append((raised, "coverage floor regressed"))
            wrong = copy.deepcopy(measured)
            wrong["measurement"]["comparison"]["profile"] = "release"
            variants.append((wrong, "not comparable"))
            for baseline, message in variants:
                (root / "baseline.json").write_text(json.dumps(baseline))
                result = subprocess.run(args, capture_output=True, text=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertEqual(result.stdout, "")

    def test_metadata_cli_preserves_cargo_test_separator(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp) / "measurement.json"
            command = ["cargo", "mutants", "--workspace", "--", "--test", "json_bounded"]
            with patch.object(sys, "argv", [quality.__file__, "metadata", str(output),
                                           "--kind", "mutations", "--command", *command]), \
                 patch.object(quality, "capture_measurement", return_value={}) as capture:
                quality.main()
            capture.assert_called_once_with("mutations", command)

    def test_short_release_flag_and_compiler_wrapper_are_recorded(self):
        from unittest.mock import patch
        compiler = "rustc 1.98.1\nhost: synthetic-host\nLLVM version: 22.1.8\n"
        with patch.object(quality.subprocess, "check_output", side_effect=[compiler, "cargo-llvm-cov 0.9.1\n"]), \
             patch.dict(quality.os.environ, {"RUSTC_WRAPPER": "/synthetic/wrapper"}):
            record = quality.capture_measurement("coverage", ["cargo", "llvm-cov", "-r"])
        self.assertEqual(record["comparison"]["profile"], "release")
        self.assertEqual(record["comparison"]["environment"]["RUSTC_WRAPPER"], "/synthetic/wrapper")

    def test_missing_floor_has_explicit_not_established_status(self):
        root = self.fixture.root
        source = quality.source_snapshot(root)
        report = root / "coverage.json"
        report.write_text(json.dumps(self.fixture.report))
        (root / "source.json").write_text(json.dumps(source))
        output = root / "summary.json"
        result = subprocess.run([*self.command, "coverage", str(report), "--snapshot", str(root / "source.json"),
            "--baseline", str(root / "missing-baseline.json"), "--output", str(output)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("coverage floor not established", result.stderr)
        self.assertEqual(json.loads(output.read_text())["floor_status"], "not_established")

    def test_failed_git_lookup_retains_a_structured_error(self):
        root = self.fixture.root
        (root / "coverage.json").write_text(json.dumps(self.fixture.report))
        (root / "source.json").write_text(json.dumps(quality.source_snapshot(root)))
        output = root / "summary.json"
        command = self.command.copy()
        command[3] = "9"
        result = subprocess.run([*command, "coverage", str(root / "coverage.json"),
            "--snapshot", str(root / "source.json"), "--output", str(output)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("quality evidence rejected", result.stderr)
        self.assertFalse(json.loads(output.read_text())["gate_passed"])
        self.assertNotIn("Traceback", result.stderr)

    def test_failed_collection_discards_previous_coverage(self):
        root = self.fixture.root
        script = root / "tests/tooling/coverage.sh"
        script.write_text((quality.ROOT / "tests/tooling/coverage.sh").read_text())
        binaries = root / "target/quality-tools/bin"
        binaries.mkdir(parents=True)
        cargo = binaries / "cargo"
        cargo.write_text("#!/bin/sh\nif [ \"$2\" = --version ]; then echo 'cargo-llvm-cov 0.9.1'; else exit 7; fi\n")
        cargo.chmod(0o755)
        compiler = binaries / "rustc"
        compiler.write_text("#!/bin/sh\necho 'rustc synthetic'; echo 'host: synthetic-host'; echo 'LLVM version: 22.1.8'\n")
        compiler.chmod(0o755)
        output = root / "target/quality"
        output.mkdir()
        (output / "coverage.json").write_text(json.dumps(self.fixture.report))
        result = subprocess.run(["sh", str(script)], cwd=root, capture_output=True, text=True)
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertFalse((output / "coverage.json").exists(), "old coverage must be discarded before collection")
        saved = json.loads((output / "coverage-summary.json").read_text())
        self.assertFalse(saved["gate_passed"])
        self.assertEqual(saved["tool_exit"], 7)
        self.assertNotIn("components", saved)

    def test_floors_reject_missing_or_incomplete_identity_even_on_both_sides(self):
        full = measurement_fixture()
        for key in list(full["comparison"]):
            incomplete = copy.deepcopy(full)
            del incomplete["comparison"][key]
            report = {"components": {}, "measurement": incomplete}
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, "not comparable"):
                quality.check_floors(report, copy.deepcopy(report))
        report = {"components": {}}
        with self.assertRaisesRegex(ValueError, "not comparable"):
            quality.check_floors(report, copy.deepcopy(report))

    def test_nonobject_mutation_report_retains_rejection_artifact(self):
        root = self.fixture.root
        (root / "outcomes.json").write_text('[]')
        (root / "source.json").write_text(json.dumps(quality.source_snapshot(root)))
        (root / "mutants.json").write_text('[]')
        output = root / "summary.json"
        result = subprocess.run([*self.command, "mutations", str(root / "outcomes.json"),
            "--snapshot", str(root / "source.json"), "--output", str(output)], capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("report must be a JSON object", result.stderr)
        self.assertFalse(json.loads(output.read_text())["gate_passed"])
        self.assertNotIn("Traceback", result.stderr)

    def test_test_runner_flags_cannot_change_recorded_build_profile_or_target(self):
        from unittest.mock import patch
        compiler = "rustc 1.98.1\nhost: synthetic-host\nLLVM version: 22.1.8\n"
        with patch.object(quality.subprocess, "check_output", side_effect=[compiler, "cargo-mutants 27.1.0\n"]):
            record = quality.capture_measurement("mutations", ["cargo", "mutants", "--", "-r", "--target", "test-filter"])
        self.assertEqual(record["comparison"]["profile"], "dev")
        self.assertEqual(record["comparison"]["target"], "synthetic-host")


if __name__ == "__main__":
    unittest.main()
