"""DEF-0056 / TC-0051: compatibility evidence must reject drift and tool failures."""

from pathlib import Path
import json
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import compatibility


class SwiftDiagnosticTests(unittest.TestCase):
    def test_zero_exit_does_not_hide_swift_api_breakage(self):
        # Observed with the installed digester on the real retained HoyaDB API:
        # it exits 0 even when -compiler-style-diags reports a removed initializer.
        result = subprocess.CompletedProcess([], 0,
            "/* Removed Decls */\nAPI breakage: constructor Database.init() has been removed\n", "")
        with self.assertRaisesRegex(compatibility.CompatibilityError, "API breakage"):
            compatibility.check_swift_result(result)

    def test_swift_tool_failure_cannot_pass(self):
        result = subprocess.CompletedProcess([], 1, "", "cannot load module")
        with self.assertRaisesRegex(compatibility.CompatibilityError, "cannot load module"):
            compatibility.check_swift_result(result)

    def test_unchanged_swift_diagnostics_pass(self):
        result = subprocess.CompletedProcess([], 0,
            "\n/* Removed Decls */\n\n/* Type Changes */\n", "")
        compatibility.check_swift_result(result)

    def test_empty_or_unrecognized_diagnostics_fail(self):
        for output in ("", "failed to load SDK", "/* Removed Decls */\nUnexpected diagnostic"):
            with self.subTest(output=output), self.assertRaises(compatibility.CompatibilityError):
                compatibility.check_swift_result(subprocess.CompletedProcess([], 0, output, ""))

    def test_only_the_exact_reviewed_breakage_is_permitted(self):
        accepted = "API breakage: enumelement HoyaError.subscriptionTerminated has been added as a new enum case"
        result = subprocess.CompletedProcess([], 0, "/* Removed Decls */\n" + accepted, "")
        compatibility.check_swift_result(result, [accepted])
        result.stdout += "\nAPI breakage: constructor Database.init() has been removed"
        with self.assertRaisesRegex(compatibility.CompatibilityError, "constructor"):
            compatibility.check_swift_result(result, [accepted])
        with self.assertRaisesRegex(compatibility.CompatibilityError, "not observed"):
            compatibility.check_swift_result(subprocess.CompletedProcess([], 0, "/* Removed Decls */", ""), [accepted])


class EvidenceTests(unittest.TestCase):
    def test_changed_removed_and_added_inputs_fail(self):
        for actual in ({"a": "changed"}, {}, {"a": "same", "b": "new"}):
            with self.subTest(actual=actual), self.assertRaisesRegex(
                    compatibility.CompatibilityError, "inputs changed"):
                compatibility.check_inputs({"a": "same"}, actual)
        compatibility.check_inputs({"a": "same"}, {"a": "same"})

    def test_missing_pattern_or_empty_file_cannot_form_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for contents in (None, ""):
                if contents is not None:
                    (root / "api.h").write_text(contents)
                with self.assertRaises(compatibility.CompatibilityError):
                    compatibility.capture(root, ["*.h"])
            (root / "api.h").write_text("void public_api(void);\n")
            self.assertEqual(set(compatibility.capture(root, ["*.h"])), {"api.h"})

    def test_import_only_wrong_module_or_malformed_swift_dump_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "api.json"
            for value in ("", "{", "{}", json.dumps({"ABIRoot": {"kind": "Root",
                    "name": "Other", "children": [{"kind": "TypeDecl"}]}}),
                    json.dumps({"ABIRoot": {"kind": "Root", "name": "HoyaDB",
                    "children": [{"kind": "Import"}]}})):
                path.write_text(value)
                with self.subTest(value=value), self.assertRaises(compatibility.CompatibilityError):
                    compatibility.check_swift_dump(path, "HoyaDB")
            path.write_text(json.dumps({"ABIRoot": {"kind": "Root", "name": "HoyaDB",
                "children": [{"kind": "TypeDecl", "name": "Database"}]}}))
            compatibility.check_swift_dump(path, "HoyaDB")

    def test_export_removal_and_unexpected_exports_fail(self):
        expected = {"hoya_v1_open", "hoya_v1_call"}
        for output in ("", "000 T hoya_v1_open", "000 T hoya_v1_open\n000 T hoya_v1_call\n000 T surprise"):
            with self.subTest(output=output), self.assertRaises(compatibility.CompatibilityError):
                compatibility.check_exports(output, expected, "android")
        compatibility.check_exports("000 T hoya_v1_open@@HOYADB_1\n000 T hoya_v1_call@@HOYADB_1\n000 A HOYADB_1",
                                    expected, "android")

    def test_apple_static_library_checks_boundary_and_rejects_new_boundary(self):
        compatibility.check_exports("000 T _hoya_v1_open\n000 T _rust_internal",
                                    {"_hoya_v1_open"}, "apple")
        with self.assertRaises(compatibility.CompatibilityError):
            compatibility.check_exports("000 T _hoya_v1_open\n000 T _hoya_v2_surprise",
                                        {"_hoya_v1_open"}, "apple")

    def test_malformed_export_evidence_is_rejected(self):
        with self.assertRaises(compatibility.CompatibilityError):
            compatibility.check_exports("hoya_v1_open", {"hoya_v1_open"}, "android")

    def test_llvm_bitcode_unknown_address_is_valid_nm_output(self):
        compatibility.check_exports("---------------- T _hoya_v1_open\n---------------- T _rust_internal",
                                    {"_hoya_v1_open"}, "apple")

    def test_android_symbol_version_changes_cannot_hide_behind_same_name(self):
        for symbol in ("hoya_v1_open@@HOYADB_2", "hoya_v1_open@HOYADB_1", "hoya_v1_open"):
            with self.subTest(symbol=symbol), self.assertRaises(compatibility.CompatibilityError):
                compatibility.check_exports("000 T " + symbol, {"hoya_v1_open"}, "android")

    def test_undefined_symbol_is_not_an_export(self):
        with self.assertRaises(compatibility.CompatibilityError):
            compatibility.check_exports("000 U _hoya_v1_open", {"_hoya_v1_open"}, "apple")

    def test_failed_or_missing_tool_cannot_pass(self):
        with patch("compatibility.subprocess.run", return_value=subprocess.CompletedProcess(
                [], 1, "", "tool failed")):
            with self.assertRaisesRegex(compatibility.CompatibilityError, "tool failed"):
                compatibility.run(["compiler"])
        with patch("compatibility.subprocess.run", side_effect=FileNotFoundError("missing compiler")):
            with self.assertRaisesRegex(compatibility.CompatibilityError, "missing compiler"):
                compatibility.run(["compiler"])


class SourceManifestTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        patterns = compatibility.SOURCE_PATTERNS + compatibility.SWIFT_PATTERNS
        for relative in compatibility.capture(compatibility.ROOT, patterns):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(compatibility.ROOT / relative, target)
        self.manifest = self.root / compatibility.CURRENT
        self.manifest.parent.mkdir(parents=True, exist_ok=True)
        self.manifest.write_text(json.dumps({"schema": 1, "review": ["test fixture"],
            "inputs": compatibility.capture(self.root, compatibility.SOURCE_PATTERNS)}))

    def test_unchanged_reviewed_sources_pass(self):
        result = compatibility.check_sources(self.root)
        self.assertIn("not runtime", result["scope"])

    def test_current_surface_mutation_and_added_or_removed_input_fail(self):
        declaration = self.root / "packages/client/index.d.ts"
        original = declaration.read_bytes()
        declaration.write_bytes(original.replace(b"export class Database", b"export class RemovedDatabase"))
        with self.assertRaisesRegex(compatibility.CompatibilityError, "inputs changed"):
            compatibility.check_sources(self.root)
        declaration.unlink()
        with self.assertRaises(compatibility.CompatibilityError):
            compatibility.check_sources(self.root)
        declaration.write_bytes(original)
        (declaration.parent / "added.d.ts").write_text("export const newSurface: string;\n")
        with self.assertRaisesRegex(compatibility.CompatibilityError, "added.d.ts"):
            compatibility.check_sources(self.root)

    def test_rewriting_current_manifest_cannot_bless_changed_historical_header(self):
        header = self.root / compatibility.HEADER
        header.write_text(header.read_text().replace("hoya_v1_call", "hoya_v2_call"))
        manifest = json.loads(self.manifest.read_text())
        manifest["inputs"] = compatibility.capture(self.root, compatibility.SOURCE_PATTERNS)
        self.manifest.write_text(json.dumps(manifest))
        with self.assertRaisesRegex(compatibility.CompatibilityError, "inputs changed"):
            compatibility.check_sources(self.root)

    def test_empty_unreviewed_or_malformed_manifest_fails(self):
        for value in ("{}", "[]", "", '{"schema":1,"review":[],"inputs":{}}'):
            self.manifest.write_text(value)
            with self.subTest(value=value), self.assertRaises(compatibility.CompatibilityError):
                compatibility.check_sources(self.root)

    def test_historical_snapshot_cannot_be_rebased(self):
        history = self.root / compatibility.HISTORY / "snapshot.json"
        history.write_text(history.read_text() + "\n")
        with self.assertRaisesRegex(compatibility.CompatibilityError, "Historical snapshot.json changed"):
            compatibility.check_sources(self.root)

    def test_source_change_during_fresh_swift_capture_fails(self):
        output = self.root / "fresh"
        changed = self.root / "bindings/swift/Sources/HoyaDB/Database.swift"
        def compiler(command, commands=None):
            if "--show-sdk-path" in command:
                return subprocess.CompletedProcess(command, 0, str(self.root), "")
            if "-dump-sdk" in command:
                module = command[command.index("-module") + 1]
                shutil.copyfile(self.root / compatibility.HISTORY / f"{module}.api.json",
                                command[command.index("-o") + 1])
            if "-diagnose-sdk" in command:
                changed.write_text(changed.read_text() + "\n// concurrent edit\n")
                diagnostics = "/* Removed Decls */\n"
                if "HoyaDB.api.json" == command[-1].name:
                    diagnostics += json.loads((self.root / compatibility.SWIFT_ACCEPTED).read_text())["HoyaDB"][0]["diagnostic"]
                return subprocess.CompletedProcess(command, 0, diagnostics, "")
            return subprocess.CompletedProcess(command, 0, "", "")
        with patch("compatibility.run", side_effect=compiler):
            with self.assertRaisesRegex(compatibility.CompatibilityError, "inputs changed"):
                compatibility.check_swift(self.root, output)
        self.assertFalse(json.loads((output / "report.json").read_text())["passed"])

    def test_library_mutation_during_inspection_fails(self):
        library = self.root / "library.a"
        library.write_bytes(b"original archive")
        def nm(command, commands=None):
            library.write_bytes(b"replaced archive")
            names = compatibility.history(self.root)["apple_static_exports"]["macos-x86_64"]
            return subprocess.CompletedProcess(command, 0, "\n".join("000 T " + name for name in names), "")
        with patch("compatibility.run", side_effect=nm):
            with self.assertRaisesRegex(compatibility.CompatibilityError, "inputs changed"):
                compatibility.check_library(self.root, library, "nm", "apple", "macos-x86_64")


if __name__ == "__main__":
    unittest.main()
