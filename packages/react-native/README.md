# HoyaDB React Native package (local, unpublished)

AI-assisted consumers: use the [source integration catalog](../../AI.md) or
read `AI.md` and `skills/hoyadb-integration/SKILL.md` in the installed matching
`@hoyadb/client` peer. The skill describes application implementation with remote
SDK releases, including ID resolution and app-specific server-readiness policies.
If an older archive lacks it, use that version's released guidance/support;
a source checkout is not a consumer prerequisite. The source link and other
repository-relative links here are optional references for library development.

This TurboModule adapts the public `@hoyadb/client/host` SDK to the same one native
Rust facade used by Swift and Kotlin. It passes original complete ABI1 frames and
opaque replies, with the binary key as a separate argument. It implements no
query, persistence, queue, conflict-resolution or retry-deadline algorithm.

The reviewed dependency and execution policy is in [ACTIVATION.md](ACTIVATION.md).
Use exactly React Native0.86.3, React19.2.3 and the explicitly approved dependency
locks; local packages are not published. Generated Codegen sources, native
artifacts, installed consumer projects and runtime evidence are separate from
source. Current source is under implementation; final platform acceptance has
not yet been recorded.

```js
import {openNativeDatabase, j, query} from '@hoyadb/react-native';
const db = await openNativeDatabase({
  name: 'notes-v1', encryptionKey: suppliedKey32,
  schema: callerSchema,
  context: () => j.string('trusted-identity'),
  clock: () => Date.now(),
});
```

The protected RN profile selects the application's controlled `hoyadb` child in
Application Support on iOS, or Android's no-backup/files parent according to
`backupExcluded` (defaulttrue). It rejects arbitrary native roots. Original
Rust validation and key/name checks precede OS root getters; rewritten-frame
validation precedes child creation and backup changes. Android OS getters may
create their managed parent. Backup policy is revalidated on each open. On Android
the host holds the Kotlin SDK's `BackupPolicyLock` from the opposite-parent check
until the key-bearing open replies, and rejects `BackupPolicyFailure` when the
store or one of its `-wal`/`-shm`/`-journal` files exists under the other policy
parent; a rejected open creates only the lock file and its no-backup `hoyadb`
child (checked on a host JVM with stand-ins, not yet on a device). There
is no plaintext fallback, key derivation, rotation or arbitrary-store import.
`openMemoryDatabase` uses the real in-memory engine without a key or filesystem
policy; production namespaces still come from native OS CSPRNG. Its optional
`store` selects a retained store from `runtime.createMemoryStore()`; omitting it
opens a new ephemeral store. Both factories pass store open options as
`options`, as the shared `OpenOptions` does.

All general operations, command scopes, managed observations, entity merge hooks
and sync scheduling are supplied by the shared JavaScript SDK. Native uptime is
a monotonic unsigned decimal string, separate from the application's UTC clock.
The native payload-free admission-release event wakes only an existing checked
core attempt; it grants no admission and carries no operation, identifier or
application data. Android uses the common `HoyaNativeTransport` AAR executor and
broker. Apple uses `dev.hoyadb.admissionReleased.v1` after the C collector and
registry guard have returned, shared with the Swift SDK.

The same host accepts optional `conflict` and `onResolution`, and exposes
`setConflictPolicy`, `reconcileResolving`, `rollbackResolving` and `discard`.
Default Stop and all core guards remain in force. See the
[shared command-conflict guide](../../tools/javascript/README.md#command-conflict-policy)
for report semantics and callback lifetime. Host regression results do not
certify an Android or iOS consumer; those require their actual native artifact.

Native module invalidation removes its release registration and closes its own
native handles, including a late successful open. One native runtime/library is
linked per process. The Android SDK Maven artifact must be supplied from its
reviewed local repository; do not package a second JNI copy. The iOS package
includes the locally built approved-source HoyaNative XCFramework.

Development checks:

```sh
node --test packages/react-native/test/*.test.mjs
python3 -I packages/react-native/tools/android/test_module_lifecycle.py
```

The Android module lifecycle check requires `javac` and `java` on PATH. It
compiles the actual module against controlled host stand-ins and covers pending
opens, publication racing invalidation, and repeated retirement. It establishes
Java owner cleanup and settlement ordering, not JNI or device integration.

The standalone Apple host check compiles the actual native transport/policy
against the current Rust archive and Foundation/Security. It does not establish
Hermes, Codegen module installation or ordinary iOS/Android app conformance;
those acceptance gates require the actual external consumers in ACTIVATION.md.


On 2026-10-31, fresh Android Codegen and the actual Java module compiled against
all 80 approved runtime inputs and the rebuilt Kotlin policy-checkpoint AAR.
Approved npm sources remained unchanged. The
[Android follow-up record](../../docs/implementation/phase-seven-rn-android-followup.md)
binds the commands and hashes and identifies the remaining compiler-observation,
security decision and package-identity gates. This verifies generated signatures
and Java compatibility; an ordinary Hermes Android consumer remains unverified.
