---
id: NFR-0005
title: hoyadb's public API follows semver, and a local reset is never the default on an upgrade
type: nfr
status: accepted
owner: contact@btk-consulting.fr
created: 2026-09-18
updated: 2026-09-18
parents: [OBJ-0002]
tags: [compatibility, versioning, library]
---

# NFR-0005: hoyadb's public API follows semver, and a local reset is never the default on an upgrade

## ISO/IEC 25010:2023 characteristic

Compatibility (sub-characteristic: Replaceability), with Maintainability
(Modifiability) as a secondary characteristic.

## Planguage

- **Tag**: api-compatibility
- **Ambition**: An integrating developer can upgrade hoyadb within a major
  version without changing their code or losing their users' local data.
- **Scale**: Number of breaking public-API changes shipped outside a major
  version bump, and number of releases requiring a local data reset.
- **Meter**: Review of each release's public API surface against the
  previous release, plus the reserved-namespace cases in `TP-0001`
  (`REQ-0033`), which verify hoyadb's own bookkeeping names cannot collide
  with an application schema.
- **Past**: n/a — no releases exist.
- **Must**:
  - **0** breaking public-API changes outside a major version bump.
  - **0** releases requiring a local data reset outside a major version
    bump.
  - Where a major bump does require a reset, hoyadb informs the developer
    that it does — **100%** of such releases.
- **Target**: **0** releases requiring a reset at all, including major
  bumps. A reset is a *possible* consequence of a major bump, explicitly
  never its default (`ADR-0023`).

## Rationale

ADR-0023's answer treats compatibility as a main goal rather than an
afterthought: "keep the compatibility as a main goal, semver. We will have
custom fields and maybe tables, we must make them generic and reserved in
the name. In case of major bump, we can inform developer of possible reset
of the data. But it must not be the default solution."

This is distinct from an application's *own* schema versioning
(`REQ-0006`), even though both use the word "version."

## Open

Semver on the public API says nothing about the **on-disk format's**
compatibility across minor versions; only the major-bump reset case is
addressed. The **exact reserved prefix string** is implementation-time
detail (`REQ-0033`).

**Incremental adoption from an existing database-backed app
is unaddressed anywhere** — every migration example assumes hoyadb owned
the schema from v1. So is **multi-tenant/multi-app backend scoping**. Both disclosed, neither
designed.

## Traceability

- Parent: `OBJ-0002`.
- Under: `ADR-0023`.
- Satisfied by: `SPEC-0003`, `ARC-0001`.
- Verified by: `TC-0003`.
