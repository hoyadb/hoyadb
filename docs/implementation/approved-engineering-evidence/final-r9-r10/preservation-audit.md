Preservation audit captured 2026-10-31T02:20:33.482071+00:00. No repository files were written, staged or committed; no builds/tests were run.

The task-start manifest contains 3,289 files. Every retained `before/` file matches its recorded SHA-256. The current inventory has 3,372 files: **78 modified, 83 added, 0 deleted**, with 3,211 baseline files unchanged. Both full current-tree reads agree.

HEAD remains `035bd0f180ca73c017813f03c454d4ad85936927`. The 33,395-byte staged diff is byte-identical to `index-before.diff`, SHA-256 `e1e09035b8e8788f5b8f09aaa3b4d16634fb6b57f29cf74ea95c138fd7480c8a`. The current raw index hash is `c076dbab98943b79181916aa6a6fee23b9d5837568633f29005f39a4662f1334` and remained unchanged during this audit. No task-start raw index capture exists, so raw index byte identity across the whole task is not claimed.

Preservation comparisons:

- instructions in baseline: 2/2 unchanged; changed paths: [].
- accepted ADRs: 38/38 unchanged; changed paths: [].
- decision log: 1/1 unchanged; changed paths: [].
- historical native API snapshots: 8/8 unchanged; changed paths: [].
- all preexisting implementation documents and evidence: 2033/2033 unchanged; changed paths: [].
- all preexisting reference material: 52/52 unchanged; changed paths: [].
- native third party files except current index: 27/27 unchanged; changed paths: [].
- dependency manifests and locks: 33/33 unchanged; changed paths: [].

All 187 frozen runtime inputs match `r10-final-runtime-source.json`.

The original Apple source index is retained exactly as `docs/implementation/approved-engineering-evidence/apple-notice-index-before-20261031.json` with SHA-256 `65a591b52a3a632526c1b1f930980cc8a7f2bd561af020b7e6e8fb19eb72bde6`. The four retained policy-checkpoint evidence files still match `checkpoint-docs-verification.json`; the subsequent saved index matches the earlier finalized index (`f401663e85815725b3a3719355d1195a8ee719c662e48f05c63f597302114620`). Both provider refresh records reference their correct saved predecessor. The current index's task-interval changes consist of six archive hashes and the provider parent/derivation reference; package/license/provider-role fields and other third-party files are preserved.

The current provider index is a live artifact and still references the pre-cfg R9/R10 parent at this capture. The newly added final parent does not overwrite either historical parent. The Apple lane may append final evidence and update the current pointer after this audit; this is not a platform certification.

No unexpected instruction or historical-evidence mutation was found in the captured scope. AGENTS.md, CLAUDE.md, accepted ADR bodies, DECISIONS.md and all preexisting implementation evidence are unchanged. Current API contracts/specifications/tests were intentionally edited for the approved work and are listed individually in the JSON; historical API snapshots remain unchanged.

Exact changed paths and before/after hashes are in `final-preservation-audit.json`; its SHA-256 is `73e6716f8a774bab9cded85daf22ba52991db2071ec617bb81633347f2c8bea2`. The fresh task-interval diff is `final-preservation-task.diff`, SHA-256 `fed10953de96b5c2cbeca11cad62ef96313bd47c73fa7f91ae9fff9c0416c837`. Existing `task.diff` and baseline artifacts were not overwritten.

Scope limits: tracked plus non-ignored untracked file bytes, using the same inventory rule as `make_diff.py`. Ignored generated outputs, ignored instruction directories absent from the baseline, filesystem metadata and author attribution are outside this comparison. Documentation can change after the timestamp.

Correction appended 2026-10-31T02:22:03.232291+00:00. The original narrative and capture above are retained.

The description **pre-cfg R9/R10 parent was incorrect**. Both retained parent files (`57baa8be218aff2f1f1e2c688972b8d1d8a2653585e92be3be6caba528bc3690` and `f6b92990642e5ce21daf0a048db5c17f43236ea571b22b54167b1a8332cc228e`) have identical 72-input source inventories and record the post-cfg `ledger_txn.rs` hash `87ea05ed454e5f8ba194af438eb79fe0996c583e005f59f4d2ef501586d1d329`. Their three native archive hashes are identical; among ten artifact entries, only `Info.plist` has a different hash. The parent identifies that difference as ordering in the later normal builder; this correction independently checked the manifest hashes, not both plist payloads.

The current provider index intentionally references the immutable first checked parent (`57baa8be…bc3690`) used for the original symbol derivation. It does not refer to a pre-cfg build or need repointing merely because a later generated packaging parent exists. This corrects the earlier implication that the pointer was awaiting a final-source update.

No preservation count or recorded source hash changes. Exact original audit bytes are retained in `final-preservation-audit.pre-correction.md` and `.json`; their hashes are `c6d6a933073ba0a7a5eb604839b1053961e08e3b083326aa23e811bd098ada7b` and `73e6716f8a774bab9cded85daf22ba52991db2071ec617bb81633347f2c8bea2`. The JSON has a new `corrections` field, with every prior field unchanged; corrected JSON SHA-256 `89b85488266bc3b7af074d9bd4fa79e02205f3caa9af3b6910ddc98feb1a6355`.
