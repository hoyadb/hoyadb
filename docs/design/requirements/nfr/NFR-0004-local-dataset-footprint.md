---
id: NFR-0004
title: hoyadb targets a mobile-appropriate local dataset — no numeric ceiling is set
type: nfr
status: draft
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0001]
tags: [footprint, scope, open-question]
---

# NFR-0004: hoyadb targets a mobile-appropriate local dataset — no numeric ceiling is set

## ISO/IEC 25010:2023 characteristic

Performance efficiency (sub-characteristic: Capacity).

## Planguage

- **Tag**: local-dataset-footprint
- **Ambition**: hoyadb holds a dataset small enough to live comfortably on
  a mobile device, and informs the developer about platform storage limits
  rather than silently hitting them at runtime.
- **Scale**: Maximum local dataset size hoyadb supports, and the platform
  storage limits it surfaces to the developer.
- **Meter**: **Not yet defined.** No measurement method is fixed, for the
  same reason `NFR-0001` has none: there is no implementation to measure.
- **Past**: n/a — new capability.
- **Must**: **No numeric ceiling is set.** What is decided is the *scope
  boundary*: hoyadb is explicitly **not** designed to hold a large,
  general-purpose database (`ADR-0024`).
- **Target**: **Not set**, for the same reason.

## Why the numbers are absent

`legacy/PLAN_DECISIONS.md` D16 answers this directly and qualitatively: "It's not
made to handle a whole database, but a 'small' one that can fit on mobile
if it's a mobile app. So not in the scope but at least to inform."

The decided commitments are therefore behavioural, not numeric:

1. hoyadb does **not** solve large-dataset storage — that is out of scope,
   stated so it does not creep in later.
2. Known platform storage limits on constrained runtimes (D16 names
   reported limitations around `AsyncStorage`-style storage on Android)
   should be **documented and made configurable where possible**, rather
   than silently hit at runtime.

Status is `draft` rather than `accepted` because the Scale carries no
measurable bound, matching `NFR-0001`'s treatment — not because the scope
decision itself is unsettled.

## Open

**The exact list of platform storage limits worth surfacing — which
platforms, which concrete numbers — is not fixed.** The stance is decided;
the table is not (`legacy/docs/08-non-functional/open-questions.md`).

This footprint bound interacts with an unaddressed gap:
**bootstrap/initial sync** for a fresh install is unspecified, including
how a large first sync would interact with this small-dataset framing
(`RISK-0005`).

## Traceability

- Parent: `OBJ-0001`.
- Under: `ADR-0024`.
- Satisfied by: nothing yet — deliberately. No measurable bound exists for
  a design artifact to satisfy.
- Verified by: nothing yet — deliberately, for the same reason.
