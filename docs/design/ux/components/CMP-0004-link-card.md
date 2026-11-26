---
id: CMP-0004
title: Link card
type: component
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
tags: [TOK-0001, design-system, documentation]
---

# CMP-0004: Link card
A whole-card link with an action title and one-sentence description, laid out
in a `.cards` grid. Used for primary reading paths, for example on the home page.

```html
<div class="cards">
<a class="card" href="getting-started.html"><strong>Start with a working database →</strong><span>Follow a complete JavaScript Todo app…</span></a>
</div>
```

## Props
| Name | Type | Required | Default |
| --- | --- | --- | --- |
| `href` | local page link | yes | — |
| `title` | verb-led action ending in " →" | yes | — |
| `description` | one sentence | yes | — |

## Events
None; native link activation.

## Slots
None.

## Visual states
- Default: `color.border.subtle` border, `radius.large`, `space.card-padding`;
  title in `color.accent.default`, description in `color.text.muted`.
- Focus: `color.focus.ring` outline (shared link rule).
- Grid: two equal columns with `space.card-gap`; one column at or below
  `layout.breakpoint-narrow`.

The card has no separate hover style; adding one requires a new token.

## Accessibility contract
- One native `<a>` per card; its accessible name is the title plus description.
- Do not nest links or buttons inside a card.
- The title states the destination action, so the link makes sense out of
  context (SC 2.4.4).

## Tokens
`color.text.default`, `color.text.muted`, `color.accent.default`,
`color.border.subtle`, `color.focus.ring`, `radius.large`,
`space.card-padding`, `space.card-gap`, `layout.breakpoint-narrow`
