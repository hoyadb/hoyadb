---
id: CMP-0001
title: Documentation page shell
type: component
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
tags: [TOK-0001, design-system, documentation]
---

# CMP-0001: Documentation page shell
The frame shared by every generated documentation page: skip link, site header,
sidebar, article, pager and provenance footer. `docs/public/build.py` renders it
from `site.json`; page bodies in `pages/*.html` supply only the article content.

## Props
| Name | Type | Required | Default |
| --- | --- | --- | --- |
| `title` | `string` (page `title` in `site.json`) | yes | — |
| `summary` | `string` (lead paragraph and meta description) | yes | — |
| `group` | `"Start" \| "Concepts" \| "Guides" \| "Reference"` (eyebrow and sidebar group) | yes | — |
| `previous` / `next` | adjacent page in `site.json` order | no | omitted at either end |
| `commit` / `date` | source commit (12 characters) and build date | yes | — |

## Events
None. Every element is a native link or the search input (CMP-0002).

## Slots
| Name | Purpose |
| --- | --- |
| `article` | Page body from `pages/<slug>.html`, placed after the lead paragraph |

## Visual states
- Wide: header row; `layout.sidebar-width` sidebar on `color.surface.subtle`
  beside the article, within `layout.max-width`.
- Narrow (≤ `layout.breakpoint-narrow`): sidebar above the article, links in two
  columns, the header "Tested examples" link hidden, `space.gutter-narrow` gutters.
- Print: header, sidebar and pager hidden; code wraps.
- Skip link: off-screen until focused, then shown at the top left.

## Anatomy
- Header: brand link (brand mark plus "hoyadb" in `font.weight.strong` and "docs"
  in `color.text.muted`), version badge "Unreleased · local packages" with
  `radius.pill`, and "Tested examples ↗" pushed to the end.
- Brand mark: 34 × 36 px, `color.accent.default` background,
  `color.accent.on-accent` italic `font.family.mark` letter "h", corner radii
  `10px 10px 18px 10px` (asymmetric corner; component-local value).
- Article: eyebrow `<group> / hoyadb`, h1 title, lead, body, pager, footer
  "Source <commit> + uncommitted working-tree changes · Built <date> ·
  Verification and limitations".

## Accessibility contract
- Landmarks: `header`, `aside`, `nav aria-label="Documentation"`, `main
  id="main" tabindex="-1"`, `nav aria-label="Adjacent pages"`; `html lang="en"`
  (SC 1.3.1). `build.py` fails the build when `lang="en"`, the documentation
  navigation label, `id="main"`, the skip link, the availability badge, commit or
  build date is missing.
- First focusable element is "Skip to content", targeting `#main` (SC 2.4.1).
- Focus uses `color.focus.ring` at `focus.width` and `focus.offset` (SC 2.4.7).
- One h1 per page; headings stay in order (SC 1.3.1).
- Content reflows at 320 CSS px without horizontal page scrolling; only code and
  tables scroll (SC 1.4.10). `docs/public/browser-check.json` (2026-10-28,
  current `style.css` hash) records all 35 pages at 320 px without overflow.
- Without JavaScript all navigation and content remain available.
- The current page is not marked in the sidebar (no `aria-current`); adding it
  is a future change, not current behavior.

## Tokens
`color.text.default`, `color.text.muted`, `color.accent.default`,
`color.accent.hover`, `color.accent.on-accent`, `color.surface.page`,
`color.surface.subtle`, `color.border.subtle`, `color.focus.ring`,
`font.family.sans`, `font.family.mark`, `font.weight.strong`,
`font.weight.heading`, `font.line-height.title`, `radius.pill`, `focus.width`,
`focus.offset`, `layout.*`, `space.gutter-narrow`
