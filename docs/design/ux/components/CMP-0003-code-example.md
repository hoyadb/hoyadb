---
id: CMP-0003
title: Verified code example
type: component
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
depends_on: [SPEC-0041, SPEC-0043]
tags: [TOK-0001, design-system, documentation, executable-examples]
---

# CMP-0003: Verified code example
A captioned, build-time highlighted code block whose source is included from a
real example file and whose caption states how that source was verified.
Rendered by `include` in `docs/public/build.py` from `{{example:<name>[:<region>]}}`.

## Props
| Name | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | example key in `language-examples.json` or the example inventory | yes | — |
| `region` | named `docs:start` / `docs:end` region | no | whole file |
| `language` | derived from the file path by `highlight.language`; labels in `highlight.LABELS` (JavaScript, TypeScript, TypeScript / TSX, Rust, Swift, Kotlin, C, JSON, Shell) | derived | build fails for plain text |
| `status` | `"executed"` or `"source-checked"` | no | `"executed"` |
| `reason` | why a block is not executed; forbidden when executed | when source-checked | — |

## Events
None.

## Slots
None. Displayed source is never handwritten; it comes from the example file.

## Visual states
- Executed: caption "Executed example · <Language> · <path> · verification"
  (an example may override the label with its `execution_label`).
- Source-checked: caption "Source-checked · not compiled or executed · <Language>
  · <path> · verification · <reason>".
- Long lines: the block scrolls horizontally; the page does not (SC 1.4.10).
- Narrow: `font.size.code-narrow` and `space.code-padding-narrow`.
- Print: code wraps (`white-space: pre-wrap`).

## Accessibility contract
- `<figure>` with `<figcaption>` naming language, file and verification status in
  text, so status never depends on color (SC 1.4.1).
- Highlighted spans keep the exact, selectable source; every syntax color has at
  least 7.68:1 against `color.code.background` (SC 1.4.3).
- `<pre>` carries `data-example`, `data-language` and `data-status` for checks.
- Caption links lead to the complete file and the verification entry.

## Tokens
`color.code.background`, `color.code.text`, `color.syntax.*`,
`color.text.muted`, `font.family.mono`, `font.size.code`,
`font.size.code-narrow`, `font.line-height.code`, `radius.large`,
`space.card-padding`, `space.code-padding-narrow`
