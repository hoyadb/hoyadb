---
id: SPEC-0051
title: Current public source snapshot
type: spec
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-26
updated: 2026-11-26
parents: [ARC-0001]
satisfies: [REQ-0048, NFR-0005]
depends_on: [SPEC-0045, SPEC-0050]
tags: [distribution, documentation]
---

# Public source snapshot

The owner authorized removal of implementation archives and prior-art comparisons,
retention of current technical contracts, incorporation of reviewed dependency
updates, and replacement of local and public main history with one clean root.
This overrides earlier preservation requirements for the removed public material.
Unrelated working-tree edits remain outside the published snapshot.

Acceptance: remove report/research trees and obsolete implementation plans;
describe capabilities without development-stage labels or product comparisons;
keep stable contract IDs, API behavior, source tests and third-party license bytes.
Move required compatibility baselines and notice-validation inputs to maintained
tool/fixture locations, preserving their hashes and rejection behavior. Update
incoming links and regenerate traceability. Review immutable dependency-update
pins and run applicable checks before replacing history with a lease against the
observed remote main. Keep a verified external recovery bundle and local edits.
No package publication or native runtime certification is implied.
