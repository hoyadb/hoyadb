---
id: DEF-0066
title: Kotlin protected open lacks the backup-policy lock and opposite-parent check
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0036]
affects: [API-0011, SPEC-0033]
regression_for: [TC-0040]
tags: [android, security, review, unverified]
---

# DEF-0066: Kotlin protected open lacks the backup-policy lock and opposite-parent check

## Symptom

`Database.openProtected` selected `noBackupFilesDir` or `filesDir` from the caller's flag with no process-wide semaphore, no file lock and no opposite-parent check. Reopening a store name with the flag flipped silently created a second, empty encrypted store, and `BackupPolicyFailure` was not an accepted error kind.

## Acceptance

As API-0011 requires: the wrapper holds the process-wide semaphore and a no-follow lock on `noBackupFilesDir/hoyadb/.hoyadb-backup-policy.lock` from the opposite-parent check through the key-bearing open reply, and rejects `BackupPolicyFailure` when any store family file exists under the opposite policy parent.

## Root cause and fix

`BackupPolicyLock.kt` implements the lock and check; `openProtected` uses it and `BackupPolicyFailure` joins the accepted error kinds. Three `BK_*` cases were added to `ProtectedOpenTests.kt`.

**Not verified at runtime.** No Android device or emulator was attached on 2026-11-10: the change compiles through `bindings/android/tools/build.py` in both modes, and the new cases have never executed. This record stays open until they run on a device. The React Native Android host only rejects an invalid directory and still lacks the same lock.

## Follow-up — 2026-11-10

The React Native Android host now reuses `BackupPolicyLock`, holding it from the opposite-parent check until the transport reply. Its five module-lifecycle cases run the real module against stand-ins on a host JVM (9 tests OK); it was not compiled against React Native or run on a device. The only available emulator image belongs to another project and was not used, so the Kotlin `BK_*` cases remain unexecuted and this record stays open.
