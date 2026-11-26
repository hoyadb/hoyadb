#!/usr/bin/env python3
"""Check the local HoyaNative XCFramework is consumable by SwiftPM and CocoaPods.

TC-0036 G23 packaging acceptance. Read-only: builds, fetches and publishes
nothing. Pass a framework path to check a packed copy; defaults to the
SwiftPM artifact produced by build-artifacts.py. DEF-0030: every copy is
checked against the trusted compilation parent SHA256 given with
--expected-parent-sha256; the records shipped inside the copy never certify
themselves.

  check_artifacts.py --expected-parent-sha256 HEX [FRAMEWORK] [TEST...]
"""
import importlib.util
from pathlib import Path
import plistlib
import shutil
import subprocess
import sys
import tempfile
import unittest

LIBRARY = "libhoyadb_native.a"
SLICES = {
    "macos-x86_64": ("macos", None, ["x86_64"]),
    "ios-arm64": ("ios", None, ["arm64"]),
    "ios-arm64_x86_64-simulator": ("ios", "simulator", ["arm64", "x86_64"]),
}
framework = Path(__file__).resolve().parent / "Artifacts/HoyaNative.xcframework"
native = Path(__file__).resolve().parents[2] / "crates/hoyadb-native"
expected_parent_sha256 = None


class XCFrameworkPackaging(unittest.TestCase):
    def setUp(self):
        self.info = plistlib.loads((framework / "Info.plist").read_bytes())
        self.libraries = {item["LibraryIdentifier"]: item for item in self.info["AvailableLibraries"]}

    def test_claimed_slices_only(self):
        self.assertEqual(set(self.libraries), set(SLICES))
        for identifier, (platform, variant, architectures) in SLICES.items():
            item = self.libraries[identifier]
            self.assertEqual(item["SupportedPlatform"], platform, identifier)
            self.assertEqual(item.get("SupportedPlatformVariant"), variant, identifier)
            self.assertEqual(sorted(item["SupportedArchitectures"]), architectures, identifier)

    def test_every_slice_uses_one_static_library_basename(self):
        # CocoaPods rejects vendored static XCFrameworks whose slices differ in
        # binary name; SwiftPM requires the lib prefix for static libraries.
        for identifier, item in self.libraries.items():
            self.assertEqual(item["LibraryPath"], LIBRARY, identifier)
            self.assertEqual(item["BinaryPath"], LIBRARY, identifier)
            self.assertTrue((framework / identifier / LIBRARY).is_file(), identifier)

    def test_headers_and_module_map_present_per_slice(self):
        # The module map sits in a HoyaNative/ subdirectory: <hoyadb.h> keeps its include path, and
        # clang stops at the first HoyaNative/module.modulemap instead of loading every copy.
        for identifier, item in self.libraries.items():
            headers = framework / identifier / item["HeadersPath"]
            files = sorted(str(p.relative_to(headers)) for p in headers.rglob("*") if p.is_file())
            self.assertEqual(files, ["HoyaNative/module.modulemap", "hoyadb.h"], identifier)

    def test_two_package_manager_copies_import_one_module(self):
        # A mixed app sees one copy via SwiftPM and one via CocoaPods' XCFramework intermediates.
        headers = framework / "macos-x86_64" / self.libraries["macos-x86_64"]["HeadersPath"]
        sdk = subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()
        with tempfile.TemporaryDirectory() as scratch:
            copies = [Path(scratch) / name for name in ("swiftpm-include", "cocoapods-headers")]
            for copy in copies:
                shutil.copytree(headers, copy)
            source = Path(scratch) / "probe.swift"
            source.write_text("import HoyaNative\nlet version = hoya_v1_abi_version()\n")
            argv = ["xcrun", "swiftc", "-typecheck", "-sdk", sdk, "-module-cache-path", str(Path(scratch) / "cache")]
            argv += [flag for copy in copies for flag in ("-I", str(copy))] + [str(source)]
            result = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=300)
            self.assertEqual(result.returncode, 0, result.stdout)

    def test_notice_packaging_closure(self):
        # Native members match the trusted compilation parent; the parent copy, NOTICE and corpus match
        # the source corpus; packaging.json lists exactly every other member. No packed-copy skip.
        if expected_parent_sha256 is None:
            self.fail("expected compilation parent SHA256 is required (--expected-parent-sha256)")
        spec = importlib.util.spec_from_file_location("apple_notices", native / "tools/apple_notices.py")
        notices = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(notices)
        try:
            summary = notices.validate_framework(framework, native / "third-party", native / "Cargo.lock",
                                                 expected_parent_sha256)
        except notices.NoticeError as error:
            self.fail(str(error))
        print(summary, file=sys.stderr)


if __name__ == "__main__":
    if sys.argv[1:2] == ["--expected-parent-sha256"] and len(sys.argv) > 2:
        expected_parent_sha256 = sys.argv[2]
        del sys.argv[1:3]
    if len(sys.argv) > 1 and Path(sys.argv[1]).is_dir():
        framework = Path(sys.argv.pop(1)).resolve()
    print("checking " + str(framework), flush=True)
    unittest.main(verbosity=2)
