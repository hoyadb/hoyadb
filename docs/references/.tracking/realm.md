---
source: https://github.com/realm/realm-js/tree/community (local model); web.archive.org (historical Device Sync docs)
source_type: source
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: realm

**Scope decision (2026-09-13, user-confirmed)**: Device Sync is deprecated/
removed (see Notes) but not dropped outright — import the still-current
local object model/schema/relations from the `realm-js` `community` branch
(sync code stripped, actively usable), plus one clearly-flagged historical
note on how Device Sync's conflict resolution worked, sourced from archived
docs. Not treated as a live precedent to imitate the way RxDB/WatermelonDB
sync are — treated as historical context only.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| Object schema/relationships (realm-js community branch) | source | imported | realm/architecture.md#object-schema, realm/architecture.md#relationships | — |
| Live objects/reactive queries (realm-js community branch) | source | imported | realm/architecture.md#live-objects-and-reactivity | — |
| Device Sync conflict resolution (historical, archived docs) | document | imported | realm/sync-historical.md | — |

## Notes

- **Blocking discovery (2026-09-13)**: Atlas Device SDKs (which is what
  "Realm" now is, including the JS SDK) were deprecated as of September
  2024 and reached end-of-life on **September 30, 2025** — confirmed
  directly from MongoDB's own docs
  (https://www.mongodb.com/docs/atlas/device-sdks/deprecation.md, fetched
  2026-09-13): "Atlas Device SDKs are deprecated... will reach end-of-life
  and be removed on September 30, 2025." That date is in the past relative
  to today (2026-09-13).
  - **Atlas Device Sync specifically is deprecated/removed** — this was the
    unit this project most wanted from Realm (sync protocol + conflict
    resolution design), per the original import plan.
  - The **on-device (local) database continues as an open-source project**,
    but the docs site's own navigation/index no longer surfaces it in the
    normal MongoDB docs index (confirmed: https://www.mongodb.com/docs/llms.txt,
    the documentation index meant for exactly this kind of discovery, lists
    no Realm/Device SDK content at all as of this fetch).
  - Each SDK (JS, Swift, Kotlin, .NET, C++, Flutter) has a "community"
    branch on GitHub with Device Sync code stripped out — e.g.
    https://github.com/realm/realm-js/tree/community — which is likely the
    only place the current, accurate local-database design still lives.
  - Repeated attempts to find live, current Realm doc pages for object
    schema/relations and live-object reactivity (via
    www.mongodb.com/docs/realm/sdk/node and the llms.txt index) turned up
    no usable navigation — the previously-known docs.mongodb.com/realm
    doc-site structure this project's import plan assumed may itself be
    gone or restructured post-deprecation.
- **Stopped here rather than guessing** (per PRIN-7 / this skill's "if
  ambiguous, ask rather than guessing") because this materially changes
  what's worth importing from Realm: Device Sync is now dead technology,
  not an actively-maintained precedent like RxDB's or WatermelonDB's
  replication protocols. Continuing to import the local-object-model
  portion is still plausible (from the GitHub `community` branch source, as
  a `source`-type unit instead of `doc-site`), but that's a scope change
  from the original plan worth confirming before spending more effort.
- **User decision (2026-09-13)**: "Local model + historical sync note" —
  proceed with schema/relations/live-objects from `realm-js` `community`
  branch source, plus one clearly historical-flagged note on Device Sync's
  conflict resolution, sourced via Wayback Machine archive since MongoDB's
  live docs no longer serve this page usably (direct `curl`/WebFetch
  attempts on the live URL returned only header/footer shell; WebFetch
  itself cannot access web.archive.org at all — worked around by `curl
  --compressed` on a Wayback `id_` snapshot URL instead).
- All 3 (rescoped) units imported 2026-09-13. Consolidated candidate
  behaviors written to `realm/requirements.md`. Import complete for this
  narrowed scope; Device Sync's wire protocol/API details were not pursued
  further (dead product, not worth the effort beyond the conflict-resolution
  design note already captured).
