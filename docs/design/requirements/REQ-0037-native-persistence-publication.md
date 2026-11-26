---
id: REQ-0037
title: Native persistence publishes only validated committed state
type: requirement
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-21
updated: 2026-10-21
parents: [OBJ-0002]
mitigates: [THR-0011]
tags: [spike, native, persistence]
---

# REQ-0037: Persist before publication

WHEN persistent native open restores data, THE spike SHALL validate stored
types, boolean range and unique IDs before exposing a handle. WHEN SQLite
returns an ordinary write/commit error, THE spike SHALL retain prior memory,
notifications and active scope, return storage failure and publish no candidate
state. SQL values SHALL use parameters. This is a bounded experiment, not a
claim of encrypted storage, hostile-file safety or crash-tested durability.
