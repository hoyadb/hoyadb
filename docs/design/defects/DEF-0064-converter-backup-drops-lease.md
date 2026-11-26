---
id: DEF-0064
title: Offline converter backup copy releases the store lease
type: defect
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-10
updated: 2026-11-10
parents: [SPEC-0037]
affects: [SPEC-0037, NFR-0006]
regression_for: [TC-0041]
tags: [sqlite, converter, review, regression]
---

# DEF-0064: Offline converter backup copy releases the store lease

## Symptom

The 2026-11-10 implementation review found that `write_backup` copied the store with `fs::read(store)` while the converter held its RESERVED lease. POSIX releases every fcntl lock a process holds on a file when any descriptor of that file closes, so the lease was gone from the backup copy until COMMIT. A second process could acquire the write lease, commit, and be overwritten from the converter's cached pages. The same-process probe in the existing SI-XL05 case could not observe this.

## Acceptance

A separate process must find the store busy at `BeforeSourceWrite` and `BeforeCommit` (SPEC-0037 R11.2.3). The backup is never more permissive than the source store, and a backup path naming the store's own `-journal`/`-wal`/`-shm` sidecar is refused without leaving a file.

## Root cause and fix

`run_inner` opens one descriptor on the store before either connection and `write_backup` reads through it; declared first, it is dropped after both connections on every path. The backup is created with the store's permission bits, and a just-created backup whose device/inode equals a store sidecar is removed through the identity-checked cleanup and rejected as `InvalidSource`.

`staged::si_xl05_no_other_process_takes_the_write_lease_after_the_backup_copy` spawns a second process and reported `acquired` at both stages before the fix and `busy` after. `si_x06_backup_is_never_more_permissive_than_the_source_store` and `si_x06_backup_path_naming_a_store_sidecar_is_refused_without_writing` cover the other two rules. `cargo test -p hoyadb-storage-sqlite --features ledger-convert-tool --test convert_process --offline --locked` reported 23 passed, 0 failed. Linux and network filesystems were not exercised.
