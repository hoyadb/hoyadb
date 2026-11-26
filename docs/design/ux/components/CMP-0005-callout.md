---
id: CMP-0005
title: Callout
type: component
status: draft
owner: contact@btk-consulting.fr
created: 2026-11-04
updated: 2026-11-04
parents: [UX-0003]
tags: [TOK-0001, design-system, documentation]
---

# CMP-0005: Callout
A highlighted note for availability, limits or a constraint the reader must not
miss. It starts with a bold label sentence.

```html
<div class="callout"><strong>Unreleased library.</strong> These docs describe checked source, not a published release.</div>
```

## Props
| Name | Type | Required | Default |
| --- | --- | --- | --- |
| `label` | short bold sentence, e.g. "Unreleased library." | yes | — |
| `body` | inline prose and links | yes | — |

## Events
None.

## Slots
| Name | Purpose |
| --- | --- |
| `body` | Prose following the label; may contain links and inline code |

## Visual states
- Default only: `color.surface.subtle` background, 3 px left rule in
  `color.accent.default`, `radius.medium` trailing corners, `space.card-padding`
  horizontal padding.

There is one callout variant. Warning or error variants need new tokens and an
accessible text label; color alone must not carry the meaning (SC 1.4.1).

## Accessibility contract
- Plain `<div>` content in reading order; no role, since it is not a live
  status message.
- The bold label carries the meaning; the accent rule is decorative.

## Tokens
`color.text.default`, `color.surface.subtle`, `color.accent.default`,
`radius.medium`, `space.card-padding`
