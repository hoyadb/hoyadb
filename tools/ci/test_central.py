"""TC-0054: Central packaging validates the certified bytes before repackaging."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[2]


class CentralTests(unittest.TestCase):
    def module(self):
        spec = importlib.util.spec_from_file_location("central", ROOT / "tools/ci/central.py")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    def test_certified_files_reject_tampering_and_development_artifacts(self):
        module = self.module()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = root / "hoyadb-android-0.1.0.aar"
            artifact.write_bytes(b"synthetic artifact")
            certificate = {"development_only": False, "coordinate": "dev.hoyadb:hoyadb-android:0.1.0",
                           "files": {artifact.name: hashlib.sha256(artifact.read_bytes()).hexdigest()}}
            self.assertEqual(module.checked_files(root, certificate, "0.1.0"), [artifact])
            for version in ("0.2.0", "../0.1.0"):
                with self.assertRaises(ValueError):
                    module.checked_files(root, certificate, version)
            artifact.write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                module.checked_files(root, certificate, "0.1.0")
            certificate["development_only"] = True
            with self.assertRaises(ValueError):
                module.checked_files(root, certificate, "0.1.0")

    def test_central_pom_keeps_dependencies_and_adds_required_metadata(self):
        module = self.module()
        source = '<project xmlns="http://maven.apache.org/POM/4.0.0"><dependencies><dependency><artifactId>keep-me</artifactId></dependency></dependencies></project>'
        result = ET.fromstring(module.release_pom(source, "hoyadb/hoyadb", "HoyaDB contributors"))
        ns = {"m": "http://maven.apache.org/POM/4.0.0"}
        self.assertEqual(result.findtext("m:licenses/m:license/m:name", namespaces=ns), "MIT License")
        self.assertEqual(result.findtext("m:dependencies/m:dependency/m:artifactId", namespaces=ns), "keep-me")
        self.assertEqual(result.findtext("m:scm/m:url", namespaces=ns), "https://github.com/hoyadb/hoyadb")


if __name__ == "__main__":
    unittest.main()
