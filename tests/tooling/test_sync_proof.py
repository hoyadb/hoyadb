"""SI-Y03: positive site plus injected construction/gate negative controls."""

import unittest
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

import sync_proof


STORAGE = "storage/sync.rs"
OWNER = "command/owner.rs"
PROOF = '''
pub struct SyncDelta { parts: SyncDeltaParts }
impl SyncDelta {
    pub(crate) fn validated_exclusive(parts: SyncDeltaParts) -> Self {
        Self { parts }
    }
    pub fn into_parts(self) -> SyncDeltaParts {
        let Self { parts } = self;
        parts
    }
}
'''
PERSIST = '''
impl<S: SyncStorage> CommandDatabase<S> {
    pub(super) fn persist(&mut self) {
        if self.db.db.validated_exclusive {
            let proof = SyncDelta::validated_exclusive(parts);
            self.storage.apply_sync_delta(proof)?;
        }
    }
}
'''


class ProofSiteTests(unittest.TestCase):
    def sources(self):
        return {STORAGE: PROOF, OWNER: PERSIST}

    def reject(self, sources, message):
        self.assertTrue(any(message in error for error in sync_proof.audit_sources(sources)))

    def test_exact_gated_site_and_consuming_destructure_pass(self):
        self.assertEqual(sync_proof.audit_sources(self.sources()), [])

    def test_extra_site_anywhere_in_core_rejects(self):
        sources = self.sources()
        sources["command/extra.rs"] = "fn extra() { SyncDelta::validated_exclusive(parts); }"
        self.reject(sources, "constructor call")

    def test_duplicate_site_in_persist_rejects(self):
        sources = self.sources()
        sources[OWNER] = PERSIST.replace("let proof =", "SyncDelta::validated_exclusive(parts); let proof =")
        self.reject(sources, "constructor call")

    def test_ungated_or_negated_or_alternative_gate_rejects(self):
        for gate in ("true", "!self.db.db.validated_exclusive",
                     "other || self.db.db.validated_exclusive"):
            with self.subTest(gate=gate):
                sources = self.sources()
                sources[OWNER] = PERSIST.replace("self.db.db.validated_exclusive", gate)
                self.reject(sources, "validated Exclusive gate")

    def test_call_after_gate_or_outside_persist_rejects(self):
        sources = self.sources()
        sources[OWNER] = PERSIST.replace("let proof =", "} { let proof =")
        self.reject(sources, "validated Exclusive gate")
        sources[OWNER] = PERSIST.replace("fn persist", "fn other")
        self.reject(sources, "persist")

    def test_missing_call_or_definition_rejects(self):
        for path in (STORAGE, OWNER):
            with self.subTest(path=path):
                sources = self.sources()
                del sources[path]
                self.assertTrue(sync_proof.audit_sources(sources))

    def test_direct_extra_struct_construction_rejects(self):
        sources = self.sources()
        sources["storage/extra.rs"] = "fn extra() { SyncDelta { parts } }"
        self.reject(sources, "construction")

    def test_extra_self_construction_in_proof_impl_rejects(self):
        sources = self.sources()
        sources[STORAGE] = PROOF.replace("pub fn into_parts", "fn extra() -> Self { Self { parts } } pub fn into_parts")
        self.reject(sources, "construction")

    def test_trait_impl_cannot_hide_self_construction(self):
        sources = self.sources()
        sources[STORAGE] += "impl Clone for SyncDelta { fn clone(&self) -> Self { Self { parts } } }"
        self.reject(sources, "construction")

    def test_public_or_other_constructor_definition_rejects(self):
        sources = self.sources()
        sources[STORAGE] = PROOF.replace("pub(crate) fn", "pub fn")
        self.reject(sources, "crate-only constructor")
        sources = self.sources()
        sources["storage/extra.rs"] = "fn validated_exclusive() {}"
        self.reject(sources, "constructor definition")

    def test_alias_call_cannot_hide_extra_constructor_use(self):
        sources = self.sources()
        sources["command/extra.rs"] = "fn extra() { Alias::validated_exclusive(parts); }"
        self.reject(sources, "constructor call")

    def test_first_class_constructor_references_cannot_hide_an_extra_site(self):
        for extra in (
            "let f = SyncDelta::validated_exclusive; let p = f(parts);",
            "let p = Some(parts).map(SyncDelta::validated_exclusive);",
            "use crate::storage::SyncDelta::validated_exclusive as mk; let p = mk(parts);",
        ):
            with self.subTest(extra=extra):
                sources = self.sources()
                sources["command/extra.rs"] = "fn extra() { " + extra + " }"
                self.reject(sources, "constructor reference")

    def test_derived_duplication_or_default_construction_rejects(self):
        for derive in ("Clone", "Copy", "Default", "Debug, Clone", "std::default::Default"):
            with self.subTest(derive=derive):
                sources = self.sources()
                sources[STORAGE] = f"#[derive({derive})]\n#[doc = \"proof\"]\n" + PROOF
                self.reject(sources, "derive")
        sources = self.sources()
        sources[STORAGE] = "#[derive(Debug)]\n" + PROOF
        self.assertEqual(sync_proof.audit_sources(sources), [])

    def test_trait_implementations_are_explicitly_rejected(self):
        for trait in ("Default", "Clone", "From<SyncDeltaParts>"):
            with self.subTest(trait=trait):
                sources = self.sources()
                sources[STORAGE] += f"impl {trait} for SyncDelta {{ fn make() -> Self {{ Self {{ parts }} }} }}"
                self.reject(sources, "trait implementation")

    def test_where_clause_cannot_hide_trait_proof_impl(self):
        sources = self.sources()
        sources[STORAGE] += "impl<T> From<T> for SyncDelta where T: Into<SyncDeltaParts> { fn from(t: T) -> Self { Self { parts: t.into() } } }"
        self.reject(sources, "trait implementation")

    def test_where_clause_cannot_hide_inherent_proof_construction(self):
        sources = self.sources()
        sources[STORAGE] += "impl SyncDelta where SyncDeltaParts: Default { fn extra() -> Self { Self { parts: Default::default() } } }"
        self.reject(sources, "construction")

    def test_owner_validated_flag_is_not_a_constructor_reference(self):
        sources = self.sources()
        sources["model/database.rs"] = "struct Database { pub(crate) validated_exclusive: bool } fn f() { db.validated_exclusive = false; }"
        self.assertEqual(sync_proof.audit_sources(sources), [])

    def test_comments_strings_chars_and_lifetimes_are_not_sites(self):
        sources = self.sources()
        sources["command/noise.rs"] = r'''
// SyncDelta::validated_exclusive(parts)
/* nested /* SyncDelta { parts } */ comment */
fn noise<'a>() { let a = "SyncDelta::validated_exclusive(parts)";
let b = r###"SyncDelta { parts }"###; let c = '{'; let d = '\'';
let e: &'a str = ""; }
'''
        self.assertEqual(sync_proof.audit_sources(sources), [])

    def test_unterminated_lexical_input_fails_closed(self):
        for text in ('/* bad', '"bad', 'r##"bad', 'fn bad() {'):
            with self.subTest(text=text):
                sources = self.sources()
                sources["command/bad.rs"] = text
                self.reject(sources, "lexical")

    def test_cli_inventories_added_rust_files_and_missing_tree_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            script = Path(sync_proof.__file__)
            def run():
                return subprocess.run([sys.executable, str(script), "--root", str(root)],
                                      capture_output=True, text=True, check=False)
            self.assertNotEqual(run().returncode, 0)
            source = root / "crates/hoyadb-core/src"
            for name, text in self.sources().items():
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(text)
            result = run()
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(set(json.loads(result.stdout)["inputs"]), {STORAGE, OWNER})
            crlf = PROOF.replace("\n", "\r\n").encode()
            (source / STORAGE).write_bytes(crlf)
            self.assertEqual(json.loads(run().stdout)["inputs"][STORAGE], hashlib.sha256(crlf).hexdigest())
            (source / "extra.rs").write_text("fn extra() { SyncDelta::validated_exclusive(parts); }")
            self.assertNotEqual(run().returncode, 0)


if __name__ == "__main__":
    unittest.main()
