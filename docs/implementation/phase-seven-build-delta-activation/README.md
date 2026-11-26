# Approved build-only dependency repository preparation

Status: exact approved inputs prepared and verified; no graph or compiler run.
Updated: 2026-10-26

The operator approved the exact `565c2bc` bootstrap and R2 additions. The
[approval record](../phase-seven-build-delta-approval.json), committed first as
`3dcaee9`, preserves that answer and its limits. It supersedes the pending
approval status in the frozen [decision packet](../phase-seven-build-delta-decision/README.md)
without rewriting that historical packet. D1/D2 remain unanswered, and the
open compiler vulnerabilities and execution holds remain.

## Inputs and preparation

Root rehashed all 101 indexed entries across the bootstrap, follow-up 6 and
decision packets against their `565c2bc` blobs, then verified all 55 approved
repository inputs: six bootstrap and 49 R2 files. Three R2 metadata files
already existed byte-identically. The new plan is the original 874 entries
unchanged plus 52 new paths: **926 files, 264 binaries and 662 metadata**.

The unchanged DEF-0028 assembler prepared a fresh repository at
`target/phase-seven-rn/android/repo-v3`. It executes no subprocess or network
operation and loads no JAR or native payload. The original `repo-v2` and its
plan/record are preserved. The build controller has not been switched.

- [Preflight](preflight-verification.json) binds the packet and assembler hashes.
- [Approved additions](approved-additions.json) binds all 55 exact inputs.
- [Plan identity](plan-identity.json) records the three overlaps and approval.
- [Plan](plan.json) and [repository record](repository-record.json) preserve
  exact input paths, provenance, expected bytes and output inventory.
- [Assembly output](assembly.txt) reports the fresh preparation.
- [Verification runs](run-identity.json) bind commands, interpreter and logs.

## Verification and limits

Normal and optimized Python both verified all 926 output files against the
plan and success record. The original 874-file repository then verified again
under optimized Python with its unchanged inventory digest. No assembler
source changed, so its existing synthetic suite was not rerun for this data
preparation. The three logged commands are the checks performed here.

New plan SHA256:
`62ced3f9a2505aab20d9ebfcf0ee7ada03a852c388c7dcb0abfa84e325e9c23d`.
New inventory SHA256:
`b244c752cb24d73bd418ddbcde5c2e6510b9de7ca63129495bffa3b09d2973d1`.
The record of these checks is [outcome.json](outcome.json).

The focused Claude Opus 5.5 review passed the approval scope, exact plan union,
both repository inventories and verification-log bindings. It ran no project
code and counted no final Phase 7 round. All 55 approved inputs bind by size
and SHA256; the six bootstrap entries additionally carry SHA512. The 49 R2
entries have null SHA512, so no SHA512 guarantee is inferred for them.

This is repository preparation only. It proves neither actual Gradle selection
nor compatibility, vulnerability remediation or unchanged shipped runtime.
Those require the separately guarded graph/build checks. Old versions remain
available as previously approved inputs; availability is not selection.
No JDK, Gradle, compiler, app, native payload or runtime positive control was
executed, and no network, publication, user-store or global configuration
operation occurred.

The plan contains local source paths and depends on retained reviewed input
files. Reproduction uses the repo-owned
[assembler/verifier](../../../bindings/react-native/tools/android/prepare_repo.py)
with explicit `--inputs`, fresh `--out` and fresh `--record` paths; repeat
preparation at the existing output rejects instead of overwriting it. A plan
is not an approval: check the separate approval and immutable packet hashes
before using it. Rollback leaves both historical repositories and their
evidence intact and selects a separately approved plan; no store migration or
cleanup is part of this preparation.

The final Phase 7 gates, both clean final review rounds, coverage and the
next-phase decision remain outstanding. Zero final rounds count here.
