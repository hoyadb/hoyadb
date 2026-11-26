---
id: ADR-0024
title: No durability guarantees beyond the queue's; hoyadb targets a small, mobile-appropriate dataset
type: adr
status: accepted
decision-makers: [contact@btk-consulting.fr]
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [durability, footprint, scope]
---

# ADR-0024: No durability guarantees beyond the queue's; hoyadb targets a small, mobile-appropriate dataset

## Context and problem statement

ADR-0024 was the remaining catch-all: whether to commit to durability guarantees
beyond `ADR-0016`'s queue-specific one (fsync behaviour, broader
crash-safety), whether to set footprint or memory ceilings, and what to
declare explicitly out of scope before it creeps in later.

## Considered options

- **Commit to nothing beyond the offline queue's engine-dependent
  transactional guarantee; state the small-dataset target explicitly; and
  document-and-make-configurable known platform storage limits rather than
  solving them.**
- **Commit to specific durability guarantees** (fsync policy, crash-safety
  bounds) across all engines.
- **Set explicit numeric footprint ceilings** (max local database size,
  memory ceiling on constrained devices).

## Decision outcome

In the context of durability and footprint commitments, facing a storage
layer whose concrete engine differs per platform (`ADR-0003`), we decided
for **no guarantees beyond the offline queue's own, plus an explicit
small-dataset target and a document-and-inform stance on platform storage
limits** and against committing to specific durability guarantees and
against setting numeric footprint ceilings, to achieve commitments hoyadb
can hold on every engine rather than ones only the default engine could
meet, accepting that hoyadb makes **no** broader crash-safety promise.

### Consequences

**Good**
- Consistent with `ADR-0003`: a guarantee hoyadb cannot hold behind every
  pluggable engine is not a guarantee worth stating. The queue's own
  transactional coupling is already explicitly engine-dependent
  (`NFR-0003`).
- The scope boundary is stated rather than left to erode: hoyadb "isn't
  designed to hold a large, general-purpose database — it targets a dataset
  small enough to live comfortably on a mobile device" (`NFR-0004`).
- Known platform storage limits on constrained runtimes (ADR-0024 names
  reported limitations around `AsyncStorage`-style storage on Android) are
  to be documented and made configurable where possible, rather than
  silently hit at runtime. Inform, do not solve.

**Bad**
- **No fsync policy and no crash-safety guarantee beyond the queue's** —
  meaning ordinary local writes outside a command's transaction carry only
  whatever the engine provides, unstated by hoyadb.
- **No numeric footprint ceiling exists**, so `NFR-0004` carries a
  qualitative scale. "Small enough for a mobile device" is not a measurable
  bound, and that is the recorded state.
- **The exact list of platform storage limits worth surfacing — which
  platforms, which concrete numbers — is not fixed.** The stance is
  decided, the table is not.
- **Large-schema authoring patterns are unaddressed** — splitting a 50+
  table schema across files, avoiding command-name collisions. Every
  example in the tree is a single inline 2-3 table `defineSchema` call.

### Confirmation

Confirmed by `NFR-0003` (the queue's engine-dependent durability, which is
the one guarantee made) and `NFR-0004` (the footprint scope). There is
nothing to confirm for the guarantees deliberately not made.

## Traceability

- Parent: `OBJ-0001`.
- Realized by: `NFR-0003`, `NFR-0004`.
- Depends on: `ADR-0003`, `ADR-0016`.
