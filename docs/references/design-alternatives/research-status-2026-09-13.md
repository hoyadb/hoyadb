# Historical research checkpoint — 2026-09-13

Preserved from the superseded refined-vision draft at baseline commit
1647b178d4fb3ee925d574367d2f9d526a4bb116. The observations below describe
that research session; they have not been reverified during this migration.
This is supporting evidence, not hoyadb's current decision record.

## Status check (2026-09-13) — is any cited precedent stale?

The 8-source research under [references/](../) was fetched
2026-09-13 (see each file's frontmatter), so it should already be current,
but per CLAUDE.md's rule against restating things from memory, every
system cited as a precedent above was re-checked live this session rather
than assumed still accurate. Findings:

- **RxDB, WatermelonDB, Zero, PowerSync, cr-sqlite (vlcn.io)** — all
  actively maintained as of this session, no deprecation found. Zero
  specifically reached **1.0 GA in June 2026**, which strengthens (not
  weakens) part 5's pick — it's no longer a young/experimental precedent.
- **Realm / Atlas Device Sync** — already correctly documented in
  [references/realm/overview.md](../realm/overview.md) as
  deprecated (Sept 2024) and end-of-life (Sept 30, 2025); nothing new
  found, this was already accurate.
- **ElectricSQL** — genuinely worth flagging: the project pivoted to
  "Electric Next," narrowing scope to read-path-only sync for its initial
  release, and its separate Electric Dart client is now deprecated pending
  that scope being widened again. This isn't a correction to anything
  cited above, though — [references/electricsql/overview.md](../electricsql/overview.md)
  already describes exactly this narrowed, read-only shape (it cites the
  current `electric.ax` docs, not the older, broader vision), so every use
  of ElectricSQL as a precedent in these alternatives files (parts 2, 3, 5)
  was already describing the current, post-pivot project. Flagged here for
  visibility, not because anything needs correcting.
- **PouchDB** — not deprecated (Apache-maintained, v9.0.0 shipped), but
  increasingly viewed by the community as legacy relative to newer
  alternatives. It's never the *chosen* option in any part here — only
  cited as an alternative (schemaless data model in part 2, coarse change
  feed in part 4, peer-to-peer topology in part 6) — so this doesn't
  change any pick, but worth knowing before leaning on it further.
- **Rust storage crates** (newly relevant after part 1's Rust pick, not
  part of the original 8-source research since none of those are Rust):
  `sled` is in maintenance mode and explicitly not recommended for new
  projects; `redb` (its named successor) and `rusqlite` both show active
  2026 development. See
  [03-storage-engine/alternatives.md](03-storage-engine/) for detail.
