---
id: DEF-0063
title: Ledger observation helpers leak into production compilation
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-31
updated: 2026-10-31
parents: [SPEC-0047]
affects: [SPEC-0037, NFR-0006]
regression_for: [TC-0041, TC-0051]
tags: [build, regression, instrumentation]
---

# DEF-0063: Production ledger observation helpers

The fresh ordinary WASM build reports dead-code warnings for
`LedgerTxn::record_keys` and `metadata_keys`. Their callers are unit tests and
the `work-counters` owner diagnostics, but the methods compile unconditionally.
Workspace all-target Clippy enables the dev self-dependency's counter feature,
so it does not exercise this ordinary production configuration.

## Regression and acceptance

On the frozen R10 source, `RUSTFLAGS='-D warnings' cargo build -p hoyadb-core
--lib --target wasm32-unknown-unknown --offline --locked` fails with exit 101
specifically for those two unused methods. Evidence is
`/tmp/hoyadb-followup-20261031/ledger-diagnostic-cfg/red.log` and `red.json`.
This is a genuine production compilation failure under denied warnings, not a
missing-symbol test or a runtime behavior failure.

Gate both observation helpers to tests or the existing `work-counters` feature,
matching their callers. Do not suppress the warning or change runtime ledger
behavior. The ordinary core/memory WASM build must pass with warnings denied;
the first-image diagnostic cases must still compile and pass. The existing CI
production WASM build now denies warnings, and CONTRIBUTING records that command.
Remote GitLab execution remains unverified.

## Correction and verification

Each helper now has `#[cfg(any(test, feature = "work-counters"))]`; the source
diff is exactly those two attributes. Every caller's compilation condition
implies the helper's condition. The ordinary core/memory WASM build with
warnings denied passes, as do all 219 core unit/integration/doc cases, core
all-target Clippy and workspace formatting. The 187-input GREEN capture is
unchanged before/after and matches the working tree. Fable reviewed the source,
caller predicates and RED/GREEN evidence without findings; it did not rerun
the commands. Evidence and review are in `ledger-diagnostic-cfg/` under the
task evidence root above.

The full R10 workspace run uses the pre-attribute snapshot. Its test configurations
already enable these helpers, so this change does not alter those test paths.
The dedicated production build supplies the no-feature evidence; all-target
Clippy with dev self-dependencies is not a substitute. Platform attempts made
before the correction remain historical; final artifacts are rebuilt from the
new source capture.
