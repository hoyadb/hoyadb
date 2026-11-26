# Exact restricted-build dependency proposal

The two corrected packets are ready for the operator's decision under
ADR-0035. They remain proposals: this record approves or activates nothing.

- [Bootstrap bundle](../phase-seven-rn-bootstrap-addendum/bundle.json): two
  build-process JARs and four metadata files.
- [R2 allowlist](../phase-seven-security-remediation-followup-6/evidence/r2-allowlist-v2.json):
  21 replacement/additional build-process JARs and 28 metadata files. Three
  metadata files already exist byte-identically; 46 listed files are new.
- [Independent Claude Opus 5.5 review](independent-review.md): no readiness
  blockers, with residual risks explicitly retained. This is not one of the
  two final Phase 7 reviews.

[proposal.json](proposal.json) binds both packet identities and records the
restrictions and unresolved findings. Root rehashed both packet indexes and
all 49 R2 entries against quarantine, without mismatches, and reran the three
synthetic resolver tests normally and under optimized Python: both passed.
The active repository still verifies against its original 874-entry plan
(241 binaries, 633 metadata); none of these proposals has been activated.

The [module-signature fetch supplement](bootstrap-module-signature-fetch.json)
preserves the later HTTP 200 acquisition record independently of the frozen
bootstrap fetch log. The R2 advisory batch contains 22 queries, including the
JCTools proxy query; it does not establish versions or advisory applicability
for every modified portion of Netty.

The two applicable compiler-embedded protobuf findings remain open. Actual
LZ4-path and native-library observation, supported identities for macOS
shared-cache images, the real Gradle graphs and Android execution are still
required. No generic security acceptance is inferred from passing unit tests
or from permission to prepare this proposal.
