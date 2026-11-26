---
source: https://rxdb.info/
source_type: doc-site
fetched: 2026-09-13
updated: 2026-09-13
status: done
---

# Import: rxdb

Scoped to hoyadb's open design questions (relational-ish model, reactive
subscriptions, command-based sync, pluggable storage/transport) — not a full
crawl of every RxDB doc page. Transport-specific replication plugins
(GraphQL/Firebase/MongoDB/CouchDB/WebRTC/HTTP/WebSocket/server) are
deliberately excluded: hoyadb cares about the abstract replication protocol
(`replication.html`), not any one transport binding.

## Units

| Unit | Kind | Status | Output | Adopted by |
| --- | --- | --- | --- | --- |
| overview.html | doc-page | imported | rxdb/overview.md | — |
| rx-storage.html | doc-page | imported | rxdb/architecture.md#storage-the-rxstorage-interface | — |
| rx-query.html | doc-page | imported | rxdb/architecture.md#query-layer-rxquery | — |
| reactivity.html | doc-page | imported | rxdb/architecture.md#reactivity-model-general | — |
| replication.html | doc-page | imported | rxdb/architecture.md#replication-protocol | — |
| transactions-conflicts-revisions.html | doc-page | imported | rxdb/architecture.md#revisions-conflict-handlers-and-transactions | — |

## Notes

- Nav discovered via root page (`/`) and `/overview.html`, per skill step 2
  (doc-site: discover pages from nav, capped to a sane scope).
- Excluded on purpose (transport-specific, not relevant to the abstract
  protocol/contract question): replication-http.html, replication-server.html,
  replication-websocket.html, replication-couchdb.html,
  replication-webrtc.html, replication-graphql.html, replication-firebase.html,
  replication-mongodb.html, rx-server.html, partial-sync.html,
  rx-storage-denokv.html, rx-storage-sqlite.html, crdt.html (CRDT is an
  alternative conflict strategy worth a future look but out of scope for
  this first pass), schema-validation.html, population.html.
- All 6 units imported 2026-09-13. Consolidated candidate behaviors written
  to `rxdb/requirements.md` (spans all units, not tied to a single row).
  Import complete for the scoped unit set above.
