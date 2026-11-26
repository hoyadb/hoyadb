# Byte-exact notice storage through Git

DEF-0027 / TC-0036 G28. Preparation59ecb6b/1cf9f19 and the independent
[corrected prep approval](../phase-seven-git-preservation-prep-approved.json)
precede the executable regression and correction.

Run `python3 tests/tooling/git_provenance.py` (also checked with `python3 -O`).
The permanent test enumerates every bindings/*/third-party corpus and the exact
Android raw cleanup log. A temporary index based on HEAD runs actual Git clean
and checkout filters with core.autocrlf=input/true/false. It compares raw bytes
and verifies a source control still receives normal LF-to-CRLF conversion with
true. No actual index, worktree input or Git config is modified by the test.

[Results](results.json) preserve all behavioral RED case headers and original
local log hash, plus both GREEN outputs and the test hash. The initial121
failures comprise10 input-mode checks (clean+checkout on the five observed
paths) and111 true-mode checks (including ordinary LF notice checkout becoming
CRLF). False has no failures. No setup or missing-file error supplied the RED.
Raw local RED remains under target/phase-seven-git-preservation/red.txt.

The correction adds only text-conversion attributes for notice corpora and the
exact raw log. Upstream whitespace remains verbatim; diff/whitespace rules and
source policies are unchanged. The two JS vcpkg blobs are explicitly restored
from existing upstream CRLF working bytes, first checked against committed
record-before.json mirror_sources. Previously Git status hid that difference.
These bytes are already inside the verified packages; no package regeneration
or new runtime result is inferred. All four final Android package hashes are
rechecked unchanged against the original certificate.

The complete staged whitespace check reports the preserved upstream CRLF license
lines; it is not reported as passing. Checks of authored files pass, and the two
restored JS license diffs disappear with ignore-space-at-eol. No whitespace
check or diff attribute was disabled to suppress this upstream formatting.

The separately reported iOS dependency sidecars are outside this correction.
A later [audit clarification](../phase-seven-ios-notice-clarification.json)
recomputes the existing approved presentation-normalization map: Boost CRLF
conversion and SocketRocket's final newline were deliberately recorded before
approval, with raw identities recoverable from inventory.json license_text.
All17 approval-pinned iOS files match. Those approved copies remain unchanged;
they are presentation bytes, not verbatim upstream sidecars. This clarification
does not certify the prebuilt components' provenance or the full Phase7 gate.

## Captured primary-source license extension

Preparationc005f9a extends the same preservation rule to raw upstream license
evidence under docs/implementation/**/sources/licenses/**. Before the new
attribute, G28 reproduced seven real filter failures: StreamEx CRLF conversion
on clean/checkout with input, plus all five primary license copies changing on
checkout with true. After the scoped -text rule, the complete matrix and source
control pass normally and under -O. The [separate extension record](source-license-extension.json)
preserves the new test hash and logs. Earlier results/reviews retain their
original source cut and counts. No source license or frozen triage index was
rewritten; the approved iOS derived presentations remain unchanged.
