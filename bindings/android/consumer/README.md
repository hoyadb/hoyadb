# Independent Android AAR consumer

This is an acceptance fixture, not the SDK implementation. Its two literal
caller schemas are TC-0036 A authors/articles/tags/article_tags and B
sensors/readings. All schema/query/command/entity/merge/retry behavior calls
public dev.hoyadb API from one locally packaged AAR. There are no source imports
from crates/spikes/SDK implementation or an alternative storage engine.
Android SQLiteDatabase is used only to create a foreign plaintext negative file;
all database behavior under test uses the protected Rust provider or real memory.

The owned project is materialized outside checkout under a fresh temp directory
hoyadb-kotlin-consumer-<nonce>. A fresh dev.hoyadb.consumer.p<16hex> app ID avoids
an existing/user app collision. The activity's Java class remains explicit.
Only owned emulator5588/API37/actual16384 pages is eligible. No user AVD is touched.
The app's synthetic32byte test key and ephemeral test signing identity are fixture
material only, not production defaults or developer credentials.

## Build activation review required before first command

`tools/build-consumer.py` takes --maven-repo and --project. It verifies local
coordinate dev.hoyadb:hoyadb-android:0.1.0 AAR/POM digests, installed reviewed
compiler closure and exact approved stdlib2.1.20/annotations13.0 archives before
activation. It copies only these independent consumer sources outside checkout.
It compiles language/API2.1 using the reviewed installed2.2.20 compiler and only
AAR classes.jar + the two runtime JARs + Android platform library as classpath.
No Gradle/plugin/cache/download is used; this is honestly a manual APK build.

The generated exact command arrays use installed tools:

1. kotlinc language/API2.1/JVM1.8/no-stdlib/no-reflect, independent consumer source.
2. D8 min-api24 with consumer.jar, AAR classes.jar and both approved runtime JARs
   as complete ordinary program inputs. The iterative split DEX mode is not used.
3. aapt2 link AndroidManifest.xml against installed android-37.0/android.jar.
4. Append classes.dex and both AAR JNI slices; lib/*/libhoyadb_jni.so ZIP_STORED.
5. zipalign -P16 -f -v4; ephemeral task-only RSA signing via installed JDK keytool;
   apksigner sign/verify; zipalign -c -P16 -v4 of the signed APK.
6. Require unchanged source/tool/AAR hashes. Record commands, actual APK/AAR/JNI
   digests, public dependency coordinate, manual-build status and runtime=false.

Manifest extractNativeLibs=false requires actual uncompressed native ZIP mapping;
it is not set true to bypass the ZIP alignment gate. Existing native-lane ELF
LOAD0x4000 evidence remains separately required for both slices. x86_64 runtime
is demonstrated only after the actual APK run; arm64 stays build-only without a
real arm64 device.

## Runtime and cleanup

`tools/run-consumer.py --project <owned> --out <new evidence> --uninstall-after`
checks the manifest/APK hash, exact serial, boot and actual page size. It refuses
an already installed same app ID. It installs only that owned APK and runs full,
seed and restart modes. Each mode writes static case IDs/Boolean/error kinds,
page size and process ID to its own noBackupFilesDir JSON report. The host reads
reports with run-as, force-stops only that app, proves no remaining PID and then
uses a genuinely fresh process for restart. A900s fixture watchdog is an
operational harness bound, not a new SDK latency promise.

Seed intentionally retains an unfinished command scope after persisting a real
row/checkpoint/Sent retry. Force-stop is the crash cut; restart checks committed
rows survive, staged row does not, queue attempts/due and checkpoint recover, and
no early send occurs. Full covers both engines' literal schemas/all value kinds,
relation navigation/query operators/batch/defaults, real UI event affinity,
suspended commands/cancel, fake exact correlation/i64, seven-step memory upgrade,
actual noBackup versus explicit filesDir, wrong-key/foreign/plaintext/corruption
byte preservation, passive hook/core validation/reset, offline behavior, source
privacy, bounded event/push prefixes and late-response replacement-owner guards.

A successful complete run uninstalls only its own app when requested. A failed
run preserves its installed owned app/store and JSON evidence for diagnosis; it
is still force-stopped. Cleanup/removal is recorded, never inferred. SDK fixtures
remain separate from prior core/provider fault suites and cannot stand in for
this packaged runtime gate.

Actual results are in
[phase-seven-kotlin-final.md](../../../docs/implementation/phase-seven-kotlin-final.md):
the first runs failed (one SDK defect, two fixture ordering errors, one host-load
harness timeout). All are preserved there. The final run passed 23/23 against the
certified AAR. On failure, the report adds a static `step` index and the SDK/fixture
throw-site `class:line`, never a message, path or payload.
