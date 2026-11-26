---
id: UX-0003
title: Public library documentation reading flow
type: ux
status: draft
owner: contact@btk-consulting.fr
created: 2026-10-27
updated: 2026-10-27
parents: [ARC-0001]
satisfies: [REQ-0047]
depends_on: [API-0004, API-0011]
tags: [documentation, public-html, executable-examples]
---

# UX-0003: Public library documentation reading flow

## Audience and flow
Application developers open the home page, choose Getting started, execute the
memory example, then follow concepts and task guides into public reference.
The site works from a local file and from a static HTTP server. Sidebar groups
are Start, Concepts, Guides and Reference. Breadcrumb and previous/next links
orient the reader; a persistent link opens the source and verification page.

## States and copy
Home title: "Build locally. Sync deliberately." Availability: "Unreleased ·
local packages". Empty search: "Search documentation". Results are page title
links. No match: "No pages found. Try another term." Without JavaScript all
navigation and content remain readable. Search data is inline, with no fetch or
XHR, so searching works from file://. Each page shows the source commit and build
date. Build errors are terminal diagnostics
with file paths and exit 1; successful verification reports actual counts.
No loading or destructive state exists. Examples use task-owned memory or
fresh temporary files. No telemetry, external fonts or CDN.

## Presentation and accessibility
Reusable CSS tokens define green accent, neutral backgrounds, spacing and type.
English strings are static; translation infrastructure is deferred. Semantic
landmarks, labelled search/navigation, skip link, visible focus, headings in
order and horizontally scrollable code address WCAG 2.2 SC 1.3.1, 1.4.3,
1.4.10, 2.1.1, 2.4.1 and 2.4.7. At narrow widths navigation moves above the
article. Search is an enhancement; no keyboard trap or modal. TC-0045 checks
structure and responsive behavior; automated checks do not prove full WCAG
conformance.

## Binding application entry

Getting started starts with "Choose your binding" and links JavaScript/web,
React Native, Swift and Kotlin entry points. The primary action is "Build your
first JavaScript app". The tutorial follows one named example: schema, open one
shared db, query, observe, define a command, implement its local handler, execute,
connect backend/transport, reconcile and dispose. Each code section precedes
its explanation and shows the expected result. Public package imports appear
first; runtime test plumbing and assertions stay in the full-source/test page.
A file map explains where code lives. Local and backend handlers are explicitly
different. Platform pages distinguish executed examples from source reference.

## Language-aware code and binding entry points
The picker links "JavaScript", "TypeScript", "Rust", "Swift", "Kotlin / Android",
"React Native" and "C / native ABI". Each guide follows the same learning order
while using its actual binding APIs. Code captions show the language and either
"Executed example" with a verification link or "Source-checked · not compiled or executed"
with a reason and verification link. Readers can inspect the complete file.

Syntax uses these reusable tokens on the existing #152b23 code background:
keyword #d8b4fe, string #a7f3a1, comment #afbdb5, number #f6c177,
type #8bd5ff, callable #f5b4d2, punctuation #dce5df; base text stays #e3f1e9.
Every token is checked for contrast at least 4.5:1 (SC 1.4.3). Code font becomes
13 px on desktop and 12 px on narrow screens, retaining 1.75 line height and
horizontal code scrolling without page overflow (SC 1.4.10). Plain source remains
selectable; language and verification status are text, not color alone.

## Design system
The tokens and building blocks above are formalized in the
[design system](../README.md): TOK-0001 records every color in
`docs/public/assets/style.css`, with measured contrast ratios, and its shared
type, radius, focus, spacing and layout values, and CMP-0001 to CMP-0005 define the page shell, search, verified code
example, link card and callout. No visual value changed. TOK-0001 records one
measured gap: the search input border is 2.62:1, below SC 1.4.11's 3:1.
