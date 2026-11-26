---
id: CMP-0002
title: Offline documentation search
type: component
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
tags: [TOK-0001, design-system, documentation]
---

# CMP-0002: Offline documentation search
A labelled search field at the top of the sidebar that filters page titles from
an index inlined in each page (`window.HOYA_SEARCH`). Implemented by
`docs/public/assets/search.js`; it makes no network request and works from
`file://`.

## Props
| Name | Type | Required | Default |
| --- | --- | --- | --- |
| `index` | `Array<{ title, text, url }>` inlined by `build.py` | yes | — |
| `limit` | number of results shown | no | `12` (fixed in `search.js`) |

## Events
| Name | Payload | Fired when |
| --- | --- | --- |
| `input` (native) | field value | Each edit; results are recomputed synchronously |

## Slots
None.

## Visual states
- Empty: label "Search documentation", placeholder "Search documentation", no results.
- Results: list of up to 12 page-title links; every whitespace-separated term
  must appear in the page title or text (case-insensitive).
- No match: text "No pages found. Try another term."
- No JavaScript: the field is inert; sidebar navigation remains the full path.

There is no loading or error state: search is synchronous and local.

## Accessibility contract
- Native `<input type="search">` with a visible `<label for="search">` (SC 1.3.1,
  SC 2.5.3) and `aria-controls="search-results"`.
- Results container is `aria-live="polite"`, so result changes and the no-match
  message are announced without moving focus (SC 4.1.3).
- No keyboard trap, modal or custom key handling; results are ordinary links in
  tab order after the field (SC 2.1.1).
- Focus uses `color.focus.ring`. The field boundary uses `color.border.control`,
  which does not meet SC 1.4.11 (see TOK-0001 known gaps).

## Tokens
`color.text.default`, `color.surface.page`, `color.border.control`,
`color.focus.ring`, `font.weight.label`, `radius.control`, `focus.width`,
`focus.offset`
