# Swift current-source iOS simulator runtime

Closes the current-source iOS runtime gap left open by the
[44-test checkpoint](../phase-seven-swift-public-testing/README.md), with the
qualifiers below. This is partial Phase7 evidence, not phase completion.

## Source and artifact identity

- Every Swift source file matches the 38 recorded checkpoint hashes
  (16c15b8/cf3f03a). The only drift is the packaging change in
  [native packaging](../phase-seven-native-packaging/README.md):
  `README.md` and `build-artifacts.py` changed, and `check_artifacts.py` is new
  ([swift-source-sha256.json](swift-source-sha256.json)).
- It links the uniform-basename XCFramework. That framework's library bytes
  are identical to the checkpoint; only Info.plist differs.

## Build

The outside-checkout consumer `hoyadb-phase7-consumer-s2413p0r` imports the
public HoyaDB/HoyaDBTesting products. It was built in a fresh scratch directory
for `x86_64-apple-ios15.1-simulator` with the iPhoneSimulator 26.5 SDK, the
explicit linker isysroot and `-warnings-as-errors`, and exited 0
([command](clean-build-command.json), [output](clean-build.txt)).

The binary is `cffdc52e090d2ac278138ac7fae92dee5b2b5feae1e83a30895631d9145cf82b`
(`LC_BUILD_VERSION` platform 7 = iOS simulator, minos 15.1). An earlier
incremental build overwrote the consumer's old `.build` iOS binary; its prior
hash is in [pre-build-binaries-sha256.txt](pre-build-binaries-sha256.txt).

## Runtime

[run.py](run.py.txt) used only the task-owned simulator
`CC525A65-D03E-4F9D-8936-DD16BBFD9664` (HoyaDB-Phase7-Owned, iOS 26.5). The
user's booted simulator was not touched, and no global CoreSimulator action was
taken. Every call is bounded and recorded in order with timings
([commands.json](commands.json)):

- boot 0.8s, bootstatus 38.8s, install 19.2s;
- writer launched with stdout/stderr files;
- independent reader process;
- uninstall and shutdown both exit 0; final state Shutdown
  ([summary](summary.json)).

- Writer: [proof](writer-result.txt) is GREEN. Every TC16/TC17 line passes on
  both the memory and protected stores ([stdout](writer-stdout.txt)).
- Reader (separate process, same app container): [proof](reader-result.txt) is
  GREEN. It covers independent protected restart, wrong key leaves the store
  unchanged, lossless i64/Float rows, frozen retry and guarded mapping.

## Qualifiers

- Executed: the x86_64 simulator slice only. The arm64 simulator and arm64
  device slices are build-only. No physical device was used.
- The earlier install and launch timeouts
  (`target/phase-seven-native/ios-diagnosis/`, `ios-final-runtime.txt`) still
  have an unknown cause. This pass does not explain them, and nothing is
  inferred about them.
- This run used `simctl launch --stdout/--stderr` and a result file, not
  `--console`.
