---
source: https://web.archive.org/web/2024id_/https://www.mongodb.com/docs/atlas/app-services/sync/details/conflict-resolution/
source_type: document
fetched: 2026-09-13
---

# Realm Atlas Device Sync — Conflict Resolution (HISTORICAL — deprecated/EOL)

⚠️ **This describes a discontinued product.** Atlas Device Sync reached
end-of-life September 30, 2025 (see [overview.md](overview.md)). Recorded
here purely as historical design context — one production system's answer
to conflict resolution — not as a live precedent to imitate the way RxDB's
or WatermelonDB's sync mechanisms are. Fetched via the Wayback Machine
(live page fetch attempts failed — see
[../.tracking/realm.md](../.tracking/realm.md) Notes) since MongoDB's
current docs no longer serve this content in a readable form.

## Mechanism: operational transformation

Device Sync used **operational transformation (OT)** — described as "a set
of rules that guarantee strong eventual consistency" so that all clients
converge to the same state regardless of the order changes were made in.
This is a different family of technique from both other references studied:
RxDB's document-level conflict *handler* (pick/merge whole states) and
WatermelonDB's per-column client-wins merge are both "diff two states and
combine them" approaches; OT instead treats each change as an *operation*
transformed against concurrent operations, closer in spirit to hoyadb's own
originally-preferred "sync commands, not just resulting state" idea, though
Device Sync's operations were still low-level field/collection edits, not
domain-level intents like `add-product-to-basket`.

## The four rules (as stated in the source)

1. **Deletes always win** — a delete on one side stays deleted even if the
   other side edited the object afterward (from that side's perspective).
2. **Last update wins** — per-property, the most recent write is kept.
3. **List inserts are ordered by time** — concurrent appends from different
   devices are merged in insertion-time order rather than one clobbering
   the other.
4. **Primary keys designate identity** — two objects created independently
   with the same primary key are treated as the same object (merged), not
   as a duplicate/conflict.

## Special-cased behaviors (worth noting as edge cases any conflict system hits)

- **Counters**: naive read-increment-write on two devices concurrently
  loses updates (both read 10, both write 11, one increment is lost).
  Device Sync's answer was an explicit increment/decrement *operation*
  (not a plain value overwrite) so the merge could combine both deltas
  correctly (arrive at 12) — i.e. some operations need to carry intent
  ("add 1") rather than just a resulting value to merge correctly. This is
  the clearest concrete evidence, across all three references, that pure
  state-diffing conflict resolution has real, unavoidable gaps that need an
  operation/intent concept to close — directly relevant to hoyadb's
  preferred command-based direction.
- **Nested collections/embedded objects**: a parent-object update always
  wins over conflicting child changes; concurrently-created distinct child
  collections under the same parent are resolved by last-update-wins
  (whichever creation happened later is kept, the other's collection is
  gone) — a real data-loss case documented as expected behavior, not a bug.
- **Strings**: treated as atomic values, not merged character-by-character
  — any concurrent edit to a string is whole-value last-write-wins, no
  partial merge.
- **Dictionaries**: deleting a key is treated as an *update* (to a
  "removed" state), not a deletion — so "delete-always-wins" does *not*
  apply to dictionary key removal; a later update to that key by another
  device wins over the earlier deletion. This is a documented inconsistency
  relative to the top-level "deletes always win" rule, worth noting as a
  cautionary example of a conflict system whose rules don't compose as
  cleanly as the four-rules summary suggests.
- **Custom resolution escape hatch**: the documented workaround for needing
  different semantics (max, min, first-write-wins, etc.) was to model the
  field as a list of individual updates instead of a single value, and
  apply custom merge logic over that list in the data model — i.e. Device
  Sync itself was not customizable; achieving custom semantics meant
  changing the schema to expose more structure to the fixed OT rules.

## Relevance to hoyadb

Three points carry forward despite this being dead technology:

1. **The counter example is direct evidence for hoyadb's preferred
   command-based-sync-with-context approach**: pure entity-diffing (RxDB,
   WatermelonDB) cannot correctly merge concurrent increments without an
   explicit "this was a +1 operation" signal — exactly the kind of context
   a command carries and a diff doesn't. This is worth citing directly in
   whatever ADR eventually decides hoyadb's sync approach.
2. **Fixed, non-customizable conflict rules have real, accepted data-loss
   edge cases** (the nested-collection creation race) — a caution for
   hoyadb to decide its conflict rules deliberately and document their
   edge cases up front, rather than assume any single rule set (LWW,
   deletes-win, OT, or otherwise) is edge-case-free.
3. **"Escape hatch by remodeling data, not by extending the sync engine"**
   is a real, if awkward, pattern for a system with fixed conflict rules —
   worth deciding explicitly whether hoyadb's own sync should expose
   pluggable conflict resolution (like RxDB/WatermelonDB do) instead.
