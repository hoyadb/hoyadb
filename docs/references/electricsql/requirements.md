---
source: https://electric.ax/docs/sync/ (multiple pages — see overview.md/architecture.md/sync.md for per-page sources)
source_type: doc-site
fetched: 2026-09-13
---

# ElectricSQL — Candidate behaviors observed

All observed — not accepted as hoyadb requirements. Each entry cites the
overview.md/architecture.md/sync.md section it's drawn from.

- **Sync engine handles reads only; writes are explicitly the app's own
  responsibility**, via whatever API the app already has — the most
  structurally different read/write split in this reference set.
  (overview.md)
- **Sync unit is a single-table, URL-addressed row filter (Shape)**, not a
  named/registered server object — the definition *is* the query string.
  (architecture.md)
- **A dedicated "must-refetch" control signal** for telling a client its
  incremental position is no longer valid and it must discard and resync
  from scratch, distinct from ordinary incremental updates.
  (architecture.md#http-protocol-mechanics)
- **Sync protocol built entirely on standard, cacheable HTTP** — CDN edge
  caching, browser cache, and generic HTTP request-collapsing are cited as
  the actual scaling mechanism, not custom server infrastructure.
  (architecture.md#http-protocol-mechanics)
- **Two selectable live-update transports on the same endpoint**
  (long-polling and SSE), and **two selectable log modes** (full snapshot+
  changes vs. changes-only) — sync behavior is client-tunable via query
  parameters rather than fixed. (architecture.md#http-protocol-mechanics)
- **Writes are explicitly presented as a spectrum of patterns**, from
  simple online-only REST writes up to a fully offline
  local-database-plus-trigger-captured-changelog approach — not a single
  prescribed design. (sync.md)
- **Optimistic-write reconciliation via "watch for it in the authoritative
  stream," not a separate ack channel**: a pending local write is
  considered resolved once the equivalent change reappears through the
  normal sync/Shape stream. (sync.md)
