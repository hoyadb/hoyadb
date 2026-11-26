# Test-only split DEX acceptance before implementation

1. A valid fixed min24 test-runtime plan records exactly two approved program
   runtime JARs and no SDK/test class; SDK D8 has exactly classes.jar program input,
   both JARs classpath-only. No silent dependency/tool download.
2. Alter min-api, D8 flags/profile, an input archive digest, android.jar, D8 launcher
   or d8.jar/JDK executable digest: reject unsupported flags or derive a distinct
   identity; never reuse the earlier identity as equivalent.
3. Missing manifest, missing classes.dex, unknown/missing metadata keys, metadata
   identity mismatch, symlinked cache/file or byte-hash mismatch: static failure
   before tool execution or device push. No repair of an existing corrupt identity.
4. Validation executes under Python -O: mismatch still rejects before mocked
   subprocess activation. Fresh output rejects prior class directories.
5. Successful generation publishes only after D8 exit0 and a verified single
   runtime DEX. Failed D8/temp incomplete cache cannot become a validated identity.
6. Actual unchanged SdkTests6+TransportTests2 run against the current8 JNI in both
   full and split modes; independent manifests have both DEX hashes and exact
   CLASSPATH order. SDK tests are never loaded from a previous complete artifact.
7. Production AAR and final consumer APK build omit split flags and continue
   ordinary complete D8 program inputs + real ZIP16KiB checks/runtime.
8. Source closure is the complete file set, not an initial list: a Kotlin/Java
   source or tool script added, removed or changed under the build roots while a
   build runs rejects with source drift and writes no success manifest. Fake
   compiler activations in tooling tests only; no real compiler is run by them.
