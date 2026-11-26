# Re-review of corrected packets (read-only; Claude Opus 5.5, claude-opus-5-5)
No network/Gradle/JVM/JAR code/gpg/test execution; ZIP/JSON/hash/source only. Scripts: scope_a.py, scope_b_v2.py, repo_v2.py.

Indexes: A v2 01a1e914… 58/58, bundle e0d5e135…; B follow-up-6 3689f7bb… 38/38; allowlist cf9608e8… (49).
repo-v2 = plan 9ff3ff84… (874/874), no proposed paths.

Prior blockers: B1 RESOLVED (junit-bom 5.13.1 .pom+.module in allowlist; sidecars md5/sha1/256/512 match; GOODSIG
FF6E2C00; OSV 1 query, empty; module = platform BOM, 18 constraints, 0 deps). A1 RESOLVED (subkey 6F538074 /
primary 2FBA29D0 = 5e771f2 pair, key file dd4e4696 identical).
Prior nonblocking: B2 fixed (namespace stripped; 44 edges = 42 + bcpkix->bcutil, bcutil->bcprov; BC/jdom2 licences
recovered; component set unchanged 21). B3 acknowledged (26 v1 POMs listed, none invented); v2 336 logged
requests (262x200, 74x404), all 262 quarantine-v2 files have a logged 200 GET with matching sha256; v1/v2 251
common all byte-identical. B4 done: 23 gpg records bound to allowlist sha256 and quarantine-v2 .asc hashes; 21
GOODSIG, jose4j/jdom2 EXPKEYSIG signed before expiry; key files bound. B5 narrowed; B6 matches repo-v2; B7 exact
49-entry list, no globs, all bytes/sha256 match quarantine-v2 and v1 where present, 164 sidecar checks pass.
A2 junit-bom 5.10.2 OSV empty; A3 module.asc GOOD same pair, asc bound; A4 wording added; proposals unchanged
(6 files identical to initial cut, initial cut preserved byte-identically).

Residual (nonblocking, disclose to operator):
R1 3 allowlist entries already exist byte-identical in repo-v2 (slf4j-api-1.7.36.pom, slf4j-parent-1.7.36.pom,
   oss-parent-9.pom): net-new files 46, not stated in README.
R2 Scope A module.asc 200-request record lives only in ignored target/.../bootstrap-review/fetch-log.json entry 39
   (sha 2bc4d293… matches); committed fetch-log.json is the frozen 38-entry original.
R3 r2-netty-notice-v2.json says OSV batch of "21 closure coordinates"; actual batch 22 (incl. jctools proxy).
R4 12 R2 JAR signatures (11 Netty, slf4j) use SHA-1 digests; identity trust UNKNOWN for all keys.
R5 Resolver is a Maven-POM approximation; real Gradle graph under R1/SR-21 guards remains the gate.
R6 Open: protobuf CVE-2024-7254 / CVE-2022-3171, LZ4 pending SR-21, Netty modified code UNKNOWN,
   java-diff-utils-parent EXPKEYSIG, B6 consumers unproven.
I did not execute the resolver tests; RED/GREEN logs read only.
