---
source: https://github.com/realm/realm-js/tree/community (README/package structure), plus MongoDB's own deprecation notice
source_type: source
fetched: 2026-09-13
---

# Realm (Atlas Device SDK) — Overview

Observed, not accepted — this describes Realm, not hoyadb.

## Current status (important, checked 2026-09-13)

Per MongoDB's own docs
(https://www.mongodb.com/docs/atlas/device-sdks/deprecation.md): Atlas
Device SDKs (what "Realm" now is) were deprecated September 2024 and
**reached end-of-life on September 30, 2025** — already past as of this
import. The SDKs have two parts:

- **The on-device (embedded) database** — continues as an open-source
  project, maintained via per-language "community" branches (e.g.
  `github.com/realm/realm-js/tree/community`) with Device Sync code
  stripped out. This is what the rest of this import covers.
- **Atlas Device Sync** (the sync/conflict-resolution layer) — deprecated
  and removed. Not imported as a live precedent; see
  [sync-historical.md](sync-historical.md) for a clearly-flagged historical
  note only.

MongoDB's primary current documentation index (`mongodb.com/docs/llms.txt`)
no longer surfaces Realm/Device SDK content at all — the previously-known
`docs.mongodb.com/realm` doc-site this import originally planned to crawl is
effectively gone/unmaintained. This import instead uses the conceptual docs
still shipped inside the `realm-js` `community` branch itself
(`docs/node/*.md`).

## What Realm (the local database) is

An embedded, object-oriented database with a live/reactive object model:
objects and query results returned from Realm are references that update
in place as underlying data changes, with opt-in change-notification
listeners at three granularities (whole realm, a query's result collection,
a single object). See [architecture.md](architecture.md) for detail.

## Relevance to hoyadb's open questions

Realm remains a useful reference for **object schema and relationship
modeling** (see [architecture.md](architecture.md)) even though its sync
layer is no longer a living precedent. Treat everything about Device Sync
in this import as historical context for what one production system tried,
not as an actively maintained design to imitate the way RxDB/WatermelonDB's
sync are.
