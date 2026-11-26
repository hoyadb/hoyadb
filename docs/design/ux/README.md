# hoyadb design system

hoyadb is a library; its user experience is mostly API design (UX-0001,
UX-0002). Its one rendered interface is the public HTML documentation
(UX-0003, SPEC-0041, SPEC-0043). This directory defines the visual and
structural system for that interface and for any future hoyadb-owned surface.

## Principles
1. **Calm, technical, honest.** One green accent on neutral surfaces. Visual
   emphasis never implies support or release status that the text and
   verification report do not state ("Unreleased · local packages").
2. **Offline and self-contained.** System fonts, inline search data and local
   assets only: no web fonts, CDN, remote highlighter or telemetry.
3. **Meaning in text, not color.** Language, verification status and callout
   labels are written words; color only reinforces them (SC 1.4.1).
4. **Measured accessibility.** Text contrast is at least 4.5:1, focus and control
   boundaries at least 3:1, and pages reflow at 320 CSS px. Ratios are computed
   and recorded in TOK-0001, not asserted; automated checks do not prove full
   WCAG 2.2 conformance.
5. **Code is the content.** Code blocks are the darkest, highest-contrast surface
   and always show real, verified source (CMP-0003).

## Artifacts
| ID | Artifact | Defines |
| --- | --- | --- |
| [UX-0003](flows/UX-0003-public-documentation.md) | Reading flow | Audience, navigation, states, copy |
| [TOK-0001](tokens/TOK-0001-documentation-design-tokens.md) | Design tokens ([DTFM JSON](tokens/TOK-0001-documentation.tokens.json)) | Color, type, radius, focus, spacing, layout |
| [CMP-0001](components/CMP-0001-documentation-page-shell.md) | Page shell | Skip link, header, brand mark, sidebar, article, pager, footer |
| [CMP-0002](components/CMP-0002-documentation-search.md) | Search | Offline search field and results |
| [CMP-0003](components/CMP-0003-code-example.md) | Code example | Captioned, highlighted, verified source |
| [CMP-0004](components/CMP-0004-link-card.md) | Link card | Reading-path entry cards |
| [CMP-0005](components/CMP-0005-callout.md) | Callout | Highlighted constraint or availability note |

Base elements (headings, paragraphs, lists, inline code, tables) follow the
TOK-0001 type scale and need no separate component contract. Tables scroll
horizontally inside the article rather than widening the page.

## Voice
Copy is short, literal and specific: "Build locally. Sync deliberately.",
"No pages found. Try another term.". Card titles are verb-led actions. Limits
and unverified platforms are stated plainly next to the claim they qualify.
Terms match the [glossary](../../guides/glossary.md).

## Changing the system
- A new color, size, spacing value or variant is added to the TOK-0001 JSON and
  table first, then used in `docs/public/assets/style.css`. Do not
  inline new literals.
- A new reusable building block gets a `CMP-NNNN` contract allocated in
  [the registry](../spec/registry.yaml) before it is built.
- Record the contrast ratio of every new color pair. A visual change needs the
  browser inspection in [docs/public/README.md](../../public/README.md) rerun.
- The stylesheet is not generated from the token file, and no automated check
  compares them; reviewers compare both files in the same change.
