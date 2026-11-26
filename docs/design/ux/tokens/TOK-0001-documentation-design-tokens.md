---
id: TOK-0001
title: hoyadb documentation design tokens
type: tokens
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
tags: [design-system, documentation, public-html, accessibility]
---

# TOK-0001: hoyadb documentation design tokens

The visual decisions of hoyadb's only rendered interface, the public HTML
documentation (UX-0003, SPEC-0041, SPEC-0043). The tokens restate the values
already implemented in `docs/public/assets/style.css`; they define no new visual
appearance. The [design system overview](../README.md) explains principles and
how to change them.

## Source file
[`TOK-0001-documentation.tokens.json`](TOK-0001-documentation.tokens.json), in
the W3C Design Tokens Format Module (DTFM). The stylesheet remains handwritten;
it does not consume the token file at build time. CSS custom properties exist for
`--ink`, `--muted`, `--green`, `--line`, `--paper`, `--soft`, `--code`,
`--radius` and `--syntax-*`; other token values appear as literals in
`style.css`. Changing either file requires changing the other in the same change.

## Tokens
Contrast ratios were computed with the WCAG 2.2 relative-luminance formula on
2026-11-04. Text pairs must reach 4.5:1 (SC 1.4.3); control boundaries and focus
indicators must reach 3:1 (SC 1.4.11).

| Name | Type | Value | CSS | Description |
| --- | --- | --- | --- | --- |
| `color.text.default` | color | `#172b25` | `--ink` | Body text and headings; 14.91:1 on page |
| `color.text.muted` | color | `#51675e` | `--muted` | Lead, captions, footer, nav groups; 6.08:1 on page, 5.62:1 on subtle |
| `color.accent.default` | color | `#166345` | `--green` | Links, eyebrow, card titles, callout rule, brand mark; 7.23:1 on page |
| `color.accent.hover` | color | `#0a432d` | literal | Link hover; 11.33:1 on page |
| `color.accent.on-accent` | color | `#ffffff` | `white` | Brand mark letter; 7.23:1 on accent |
| `color.surface.page` | color | `#ffffff` | `--paper` | Page, input and skip-link background |
| `color.surface.subtle` | color | `#f3f7f3` | `--soft` | Sidebar, table header, inline code, callout |
| `color.border.subtle` | color | `#dce5df` | `--line` | Decorative dividers only; 1.29:1 on page |
| `color.border.control` | color | `#8ca697` | literal | Search input border; 2.62:1 on page (known gap) |
| `color.focus.ring` | color | `#a44709` | literal | Focus outline; 6.03:1 on page, 5.57:1 on subtle |
| `color.code.background` | color | `#152b23` | `--code` | Code block background |
| `color.code.text` | color | `#e3f1e9` | literal | Plain code text; 12.86:1 |
| `color.syntax.keyword` | color | `#d8b4fe` | `--syntax-keyword` | 8.47:1 on code |
| `color.syntax.string` | color | `#a7f3a1` | `--syntax-string` | 11.40:1 on code |
| `color.syntax.comment` | color | `#afbdb5` | `--syntax-comment` | 7.68:1 on code |
| `color.syntax.number` | color | `#f6c177` | `--syntax-number` | 9.14:1 on code |
| `color.syntax.type` | color | `#8bd5ff` | `--syntax-type` | 9.32:1 on code |
| `color.syntax.callable` | color | `#f5b4d2` | `--syntax-callable` | 8.80:1 on code |
| `color.syntax.punctuation` | color | `#dce5df` | `--syntax-punctuation` | 11.64:1 on code |
| `font.family.sans` | fontFamily | `system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif` | literal | All prose and interface text |
| `font.family.mono` | fontFamily | `ui-monospace, SFMono-Regular, Consolas, monospace` | literal | Code blocks |
| `font.family.mark` | fontFamily | `Georgia, serif` | literal | Brand mark letter only, italic |
| `font.size.base` | dimension | `16px` | literal | Root size |
| `font.size.code` | dimension | `13px` | literal | Code above the narrow breakpoint |
| `font.size.code-narrow` | dimension | `12px` | literal | Code at or below the narrow breakpoint |
| `font.weight.regular` | fontWeight | `400` | literal | Body text |
| `font.weight.label` | fontWeight | `650` | literal | Form labels |
| `font.weight.heading` | fontWeight | `720` | literal | Page title |
| `font.weight.strong` | fontWeight | `750` | literal | Brand name, eyebrow |
| `font.line-height.body` | number | `1.7` | literal | Body text |
| `font.line-height.code` | number | `1.75` | literal | Code at every width |
| `font.line-height.title` | number | `1.15` | literal | Page title |
| `font.line-height.heading` | number | `1.35` | literal | Section heading |
| `radius.large` | dimension | `12px` | `--radius` | Cards, code blocks |
| `radius.medium` | dimension | `8px` | literal | Callout trailing corners |
| `radius.control` | dimension | `7px` | literal | Search input |
| `radius.inline` | dimension | `4px` | literal | Inline code |
| `radius.pill` | dimension | `20px` | literal | Version badge |
| `focus.width` | dimension | `3px` | literal | Focus outline width |
| `focus.offset` | dimension | `4px` | literal | Focus outline offset |
| `space.card-gap` | dimension | `16px` | literal | Gap between cards |
| `space.card-padding` | dimension | `22px` | literal | Card, callout and wide code padding |
| `space.code-padding-narrow` | dimension | `15px` | literal | Narrow code padding |
| `space.gutter-narrow` | dimension | `20px` | literal | Narrow page gutter |
| `layout.max-width` | dimension | `1440px` | literal | Sidebar plus article grid |
| `layout.sidebar-width` | dimension | `270px` | literal | Sidebar column |
| `layout.article-max-width` | dimension | `1110px` | literal | Article column |
| `layout.header-min-height` | dimension | `80px` | literal | Site header |
| `layout.breakpoint-narrow` | dimension | `760px` | literal | Single-column layout threshold |

## Type scale
DTFM dimensions accept only `px` and `rem`, so these relative and fluid values
stay in this table. They are part of the token set and follow the same change rule.

| Style | Size | Weight | Line height | Tracking | Other |
| --- | --- | --- | --- | --- | --- |
| Page title (h1) | `clamp(2rem, 3.5vw, 3.4rem)` | 720 | 1.15 | `-0.045em` | wraps anywhere |
| Section heading (h2) | `1.45rem` | 700 (browser default) | 1.35 | `-0.025em` | wraps anywhere |
| Subheading (h3) | `1.07rem` | 700 (browser default) | inherited | none | wraps anywhere |
| Lead | `1.14rem` | 400 | 1.7 | none | muted, max `70ch` |
| Body / list item | `1rem` | 400 | 1.7 | none | max `78ch` |
| Brand name | `1.4rem` | 750 | inherited | none | — |
| Eyebrow | `0.7rem` | 750 | inherited | `0.15em` | uppercase, accent |
| Sidebar group heading | `0.7rem` | 700 (browser default) | inherited | `0.13em` | uppercase, muted |
| Sidebar link | `0.84rem` | 400 | inherited | none | — |
| Table | `0.85rem` | 400 | inherited | none | — |
| Inline code | `0.87em` | 400 | inherited | none | subtle background |
| Caption | `0.75rem` | 400 | inherited | none | muted |
| Footer | `0.73rem` | 400 | inherited | none | muted |

## Known gaps
- `color.border.control` measures 2.62:1 against `color.surface.page`, below the
  3:1 non-text contrast of SC 1.4.11. The search input also has a visible label,
  but its boundary alone does not meet the criterion. Changing the value is a
  visual change that needs its own regression check and browser re-inspection.
- Most dimensions are literals in `style.css` instead of custom properties, and
  no automated check compares the stylesheet with the token file.

## Consumers
- [CMP-0001](../components/CMP-0001-documentation-page-shell.md) — page shell.
- [CMP-0002](../components/CMP-0002-documentation-search.md) — search.
- [CMP-0003](../components/CMP-0003-code-example.md) — code example.
- [CMP-0004](../components/CMP-0004-link-card.md) — link card.
- [CMP-0005](../components/CMP-0005-callout.md) — callout.
