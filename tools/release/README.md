# Preparing a release

The manually dispatched [GitHub release workflow](../../.github/workflows/release.yml)
prepares checked artifacts and optionally publishes them after environment review.
Packages remain unpublished. First-party code now uses [MIT](../../LICENSE);
security reporting, registry ownership and the supported target matrix remain
external distribution gates. See [GitHub setup](../../docs/guides/github-delivery.md)
for runner, registry and Pages prerequisites and current native acceptance limits.

Run the [contribution checks](../../CONTRIBUTING.md), then validate complete
consumer artifacts from current source:

```sh
sh crates/hoyadb-web/build.sh
npm run build:packages
npm run check:packages
npm run test:packaging
```

The packaging test installs only local npm tarballs into a temporary directory,
typechecks consumers and executes the bundled WASM engine. No registry publish
occurs. Keep this independent-consumer check: importing directly from source is
insufficient to prove the artifact includes its runtime, declarations and notices.

For Apple artifacts use [the Swift build and artifact checks](../../bindings/swift/README.md).
For Android use [the AAR packaging and consumer checks](../../bindings/android/README.md).
For React Native use [its activation and packaging requirements](../../packages/react-native/ACTIVATION.md).
Keep platform-specific builders beside their implementation, and retain source
hashes, artifact checksums and notice closures with each release candidate.

The native C ABI builds static/shared libraries; fixture executables under the
storage crate are test infrastructure, not a new public CLI product. Rust crates
remain unpublished. A future crates.io release must resolve their path-only/Git
dependencies and package contents before publication is enabled. Swift's local
XCFramework target likewise needs a deliberate remote delivery arrangement.

Review versions and compatibility across Cargo, npm, Maven and Apple artifacts
against one source revision. Rebuild after a source move rather than changing an
old certificate's paths or hashes. Record executed targets separately from
build-only targets and unresolved platforms. Generate release notes and obtain
explicit publication authorization only after the concrete artifacts are ready.

## Compatibility evidence

API-0011's retained [native/Swift/React Native snapshots](../../docs/implementation/phase-seven-native-api-snapshots/)
are historical inputs. `compatibility.py` enforces them without rewriting them
(SPEC-0047 / DEF-0056 / TC-0051). Run the portable source checks and regressions:

```sh
python3 -m unittest discover -s tools/release -p 'test_*.py'
python3 tools/release/compatibility.py sources
```

The source check preserves the historical C header, module map and RN Codegen
spec exactly. `current-api.json` records an explicitly reviewed current surface
for RN declarations, factory exports, package entry points and the shared client
declarations. Added/removed declaration files and any hash drift fail. This is a
conservative review gate; unchanged hashes do not prove runtime behavior or
semantic compatibility. Existing packed-consumer type/runtime checks still apply.

To propose a source snapshot after an approved API change, write a separate
candidate, review its source diff against the relevant contract and retained
surface, and record that review before updating `current-api.json`:

```sh
python3 tools/release/compatibility.py candidate --output /tmp/hoyadb-api-candidate.json
```

Candidate generation is not a pass. It refuses to overwrite a file or write into
the retained/reviewed baseline location. Never accept changed hashes solely to
make a check pass. The historical RN factories remain available alongside the
API-0012 application overloads; changes to shared client declarations must also
be reviewed because RN re-exports them.

On macOS with Xcode's command-line tools, compile fresh Swift modules and compare
their public API to the historical digester dumps:

```sh
python3 tools/release/compatibility.py swift --output /tmp/hoyadb-swift-api
```

The output directory must not exist. The check builds the current HoyaDB and
HoyaDBTesting Swift sources against the current native C header for
`x86_64-apple-macosx13.0`, using a fresh module cache; it does not consume an old
XCFramework or `.build` module. `report.json` retains source hashes, exact
commands, diagnostics and fresh dump hashes. Inputs changed during capture,
missing/empty/malformed dumps, unknown diagnostics and failed tools all fail.
The digester can exit zero while printing `API breakage:`; these diagnostics are
checked explicitly. `swift-accepted-changes.json` lists only exact, individually
reviewed diagnostic exceptions for contract-approved source breaks. A missing
expected exception or any other diagnostic fails. This is a macOS source API
check, not binary ABI, iOS execution or runtime certification.

Inspect each actual native artifact separately, selecting a retained slice:

```sh
python3 tools/release/compatibility.py exports --kind apple --slice macos-x86_64 --library PATH_TO_ARCHIVE --nm PATH_TO_LLVM_NM
python3 tools/release/compatibility.py exports --kind android --slice x86_64 --library PATH_TO_JNI_LIBRARY --nm PATH_TO_LLVM_NM
```

Apple static archives must retain the exact `hoya_`/JNI public boundary; internal
Rust globals are outside that check. Android's entire defined dynamic symbol
set must match, apart from the `HOYADB_1` version node; each boundary function
must retain its exact default `@@HOYADB_1` version. Undefined symbols never count
as exports. Use an `nm` that can read
the artifact's LLVM format: older Apple/Android tools can print some symbols and
then fail on newer Rust bitcode. Such tool failures are never ignored. The JSON
result binds the supplied library and inspector hashes, command and export set.
An export check on an older library proves only that library's exports; it does
not establish its correspondence to current source, platform execution or
release readiness. Keep the platform builders' independent provenance checks.
Compare the complete current build input inventory, including additions and
removals, with the captured build inputs before claiming current-source coverage.
A passing source API check and a passing export check do not bind an older
library to newer implementation code. The retained
[2026-10-31 Apple policy checkpoint](../../docs/implementation/approved-engineering-evidence/README.md)
explicitly records such a later source mismatch; its passing checks remain
evidence for that checkpoint.
