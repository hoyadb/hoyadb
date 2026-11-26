"""Prepare Central metadata from a current-source, final local Maven certificate.

Does not sign, upload, change native binaries or substitute a development AAR.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[2]
NS = "http://maven.apache.org/POM/4.0.0"


def checked_files(directory, certificate, version):
    if (not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version)
            or certificate.get("development_only") is not False
            or certificate.get("coordinate") != f"dev.hoyadb:hoyadb-android:{version}"):
        raise ValueError("not a final certificate for the selected release")
    files = certificate.get("files")
    if not isinstance(files, dict) or not files:
        raise ValueError("certified files are missing")
    result = []
    for name, expected in files.items():
        path = directory / name
        if (Path(name).name != name or path.is_symlink() or not path.is_file()
                or hashlib.sha256(path.read_bytes()).hexdigest() != expected):
            raise ValueError("certified artifact changed: " + name)
        result.append(path)
    return result


def release_pom(source, repository, developer):
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) or not developer.strip():
        raise ValueError("repository/developer metadata is missing")
    ET.register_namespace("", NS)
    root = ET.fromstring(source)
    if root.tag != f"{{{NS}}}project":
        raise ValueError("unexpected Maven POM namespace")
    def element(parent, name, text=None):
        value = ET.SubElement(parent, f"{{{NS}}}{name}")
        value.text = text
        return value
    for name in ("description", "url", "licenses", "scm", "developers"):
        for old in root.findall(f"{{{NS}}}{name}"):
            root.remove(old)
    url = f"https://github.com/{repository}"
    element(root, "description", "HoyaDB offline-first embedded database Android SDK")
    element(root, "url", url)
    license_node = element(element(root, "licenses"), "license")
    element(license_node, "name", "MIT License")
    element(license_node, "url", url + "/blob/main/LICENSE")
    element(license_node, "distribution", "repo")
    scm = element(root, "scm")
    element(scm, "url", url)
    element(scm, "connection", "scm:git:" + url + ".git")
    element(scm, "developerConnection", f"scm:git:ssh://git@github.com/{repository}.git")
    author = element(element(root, "developers"), "developer")
    element(author, "id", repository.split("/")[0])
    element(author, "name", developer)
    element(author, "url", "https://github.com/" + repository.split("/")[0])
    ET.indent(root)
    return ET.tostring(root, encoding="utf-8", xml_declaration=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--maven", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--repository", required=True)
    args = parser.parse_args()
    version = json.loads((ROOT / "packages/client/package.json").read_text())["version"]
    certificate = json.loads((args.maven / "certificate.json").read_text())
    coordinate = Path("dev/hoyadb/hoyadb-android") / version
    files = checked_files(args.maven / coordinate, certificate, version)
    expected = {f"hoyadb-android-{version}{suffix}" for suffix in
                (".aar", ".pom", "-sources.jar", "-notices.zip")}
    if {path.name for path in files} != expected:
        raise ValueError("the complete certified Maven artifact set is required")
    spec = importlib.util.spec_from_file_location("aar", ROOT / "bindings/android/tools/package-aar.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if (certificate.get("sources") != module.production_sources()
            or certificate.get("native_rust_sources_sha256") != module.RUST.rust_sources(ROOT)):
        raise ValueError("Maven certificate does not describe current source")
    target = args.out / coordinate
    args.out.mkdir(parents=True, exist_ok=False)
    target.mkdir(parents=True)
    for path in files:
        shutil.copyfile(path, target / path.name)
    pom = target / f"hoyadb-android-{version}.pom"
    pom.write_bytes(release_pom(pom.read_text(), args.repository, "HoyaDB contributors"))
    # Real SDK usage documentation, not an empty validation placeholder.
    with zipfile.ZipFile(target / f"hoyadb-android-{version}-javadoc.jar", "w", zipfile.ZIP_DEFLATED) as archive:
        for source in ("README.md", "NOTICE.md"):
            archive.write(ROOT / "bindings/android" / source, source)
        archive.write(ROOT / "LICENSE", "LICENSE")
    with zipfile.ZipFile(target / f"hoyadb-android-{version}-notices.zip", "a", zipfile.ZIP_DEFLATED) as archive:
        archive.write(ROOT / "LICENSE", "LICENSE")
    print("Prepared Central artifacts; namespace ownership, signing and publication are separate gates")


if __name__ == "__main__":
    main()
