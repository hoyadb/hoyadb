# Public HTML documentation

Current user documentation for the unreleased library, separate from historical
design guides. Governed by SPEC-0041/SPEC-0043, TC-0045/TC-0047 and UX-0003. Claude assists with
writing and review; there is no in-page AI service.

From the repository root:

1. `python3 docs/public/verify.py`
2. `python3 docs/public/build.py`
3. Open `docs/public/dist/index.html` directly, or serve that directory with
   `python3 -m http.server 8080 --directory docs/public/dist`.

Prerequisites are the pinned Rust toolchain/dependencies, Node and the existing
wasm-bindgen 0.2.129 CLI/WASM compiler documented in `crates/hoyadb-web/README.md`.
The language tours also require the installed TypeScript compiler, Swift and C toolchains on macOS, plus the locally reviewed Kotlin compiler, JDK, Android SDK/NDK and task-owned Android emulator for the Kotlin guide. These scripts download nothing. They use Python's standard library, the local
library sources and the actual ordinary WASM build.

`verify.py` runs the executable examples and clippy, source-checks the explicitly held snippets, binds its results to source hashes,
and refuses a changed source tree. `build.py` includes examples from those exact
files, checks guide symbols against reference source, generates Rust API docs,
and rejects broken local links/fragments or unlabelled code blocks. Generated
`dist/` is ignored. `verification.json` records only the last successful run and
is regenerated; it is not a timeless test claim. Source-only declarations are labelled as reference. The native React Native bootstrap is an included usage file labelled **Source-checked · not compiled or executed**. Their named calls are checked against current declarations and an explicit standard-library allowlist; this lexical check is not a compiler or runtime guarantee.

Edit `pages/*.html` article bodies and `site.json` navigation. Example tokens are
`{{example:quickstart}}`; named regions use `{{example:application:open}}` with paired `// docs:start open` / `// docs:end open` markers in the executed file. Missing, duplicate and nested regions fail closed. Add real examples under `examples/src/bin/` (Rust) or
`examples/javascript.mjs` (Node), never duplicate a handwritten code block. Link
source using `href="source:crates/..."`; the builder packages escaped source for
offline reading. Update `symbols.json` when adding named guide APIs. Runtime
package installation, native/mobile releases and user stores are outside this
site build. Build output is not published automatically.

After the site build, run `node docs/public/browser-check.mjs` to check it in an
installed Chrome (override its path for both verification and browser inspection with `HOYADB_CHROME`), then rebuild with
`python3 docs/public/build.py --skip-rustdoc` to bundle the new browser report.
The browser test creates and removes its own temporary profile.

The Python tests and Node search test are executable tooling checks. A real
browser inspection is recorded in `browser-check.json`; structural checks alone
are not WCAG certification. `--skip-rustdoc` is for iteration after a successful
reference build; final validation rebuilds reference.

The application tour imports the public local packages. Verification rebuilds
those packages, runs the exact file in an isolated Node consumer, and runs it in
installed Chrome with the real dedicated worker. The browser fixture also checks
a protected OPFS reopen in its own disposable profile. No native/mobile runtime
is inferred from these checks.


## Languages and verification

The getting-started picker links JavaScript, TypeScript, Rust, Swift,
Kotlin/Android, React Native and C. `highlight.py` colors those languages plus
JSON and shell at build time, preserving exact selectable source. Unknown
reference files stay escaped plain text. There are no remote highlighter assets.

`language-examples.json` records new tour paths and execution status. Every
executed tour must reach the completion line in `example-completions.json`, after
its assertions. The build rejects a compile-only success. Source-only snippets
require a source-check result, an explicit reason and current symbol checks.

- TypeScript: `node docs/public/run_language.mjs typescript` strictly compiles,
  emits and executes a public package consumer against WASM.
- React Native shared logic: `node docs/public/run_language.mjs react-native`
  executes against WASM. The native TurboModule bootstrap remains uncompiled and
  unexecuted. This does not establish native runtime behavior.
- Swift and C: `python3 docs/public/run_native.py build`, then the `swift` and `c`
  subcommands compile and execute the tours. All native outputs and Swift package
  staging stay under ignored `target/documentation-languages/`; packaged
  XCFrameworks are never replaced. The native build report records source and
  artifact hashes. Swift/C build an archive of HEAD with a task-only standalone
  workspace stanza; Rust/WASM use the recorded working-tree sources. This
  distinction is explicit in the verification page. Kotlin has its own Android build below.
- Kotlin: `python3 docs/public/run_kotlin.py build` compiles the archived HEAD core and binding plus the guide; `python3 docs/public/run_kotlin.py run` executes it on the task-owned Android emulator (x86_64, API 37, 16 KiB pages) through `app_process`. Outputs stay under `target/documentation-languages/kotlin`; manifests record source, tool and artifact hashes. The runner starts the owned AVD read-only if needed, removes its device temporary directory and stops only the emulator it started. It records whether adb was already running and leaves the shared adb server running. There are no downloads, app installs or Gradle builds. This is not an APK build, physical-device check or D1/D2, Phase 7 or R9 certification. The wider SDK hold remains. `check_sources.py kotlin` remains an additional lexical check.

Add a new example to the manifest and completion inventory, add its runner to
`verify.py`, and update guide symbols. Never label a source-only block executed.

## Writing readable examples

Teach one operation per named region: explain its purpose before the snippet,
then describe its result or a practical constraint. Keep assertions and test-only
setup in the complete runnable file, outside those regions. Application error
handling stays visible. Prefer short blocks with blank lines between logical
steps; a short close call does not need padding. The renderer removes common
outer indentation and preserves internal formatting. Link the complete file
for readers who want to run or copy it. Reference pages can show whole files.

`test_readability.py` checks named-region use, prose before snippets, absence of
verification checks in displayed code, and indentation. Human review checks
that the explanation and snippet order are accurate and helpers are introduced.
