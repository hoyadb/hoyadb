---
source: https://github.com/Nozbe/WatermelonDB/blob/master/docs-website/docs/docs/README.md
source_type: source
fetched: 2026-09-13
---

# WatermelonDB — Overview

Observed, not accepted — this describes WatermelonDB, not hoyadb.

## What it is

A reactive database framework for React and React Native apps, built on
SQLite, designed to launch and query fast even with tens of thousands of
local records.

## Core claims (from the docs README)

- **Lazy loading**: nothing is loaded into JS until requested — the stated
  motivation is that loading a full DB into JS (as a naive Redux/MobX +
  persistence approach would) gets slow at scale, especially on slower
  Android devices.
- **SQLite-backed, off the JS thread**: queries run directly against SQLite
  on a separate native thread; most queries "resolve in an instant."
- **Relational**: explicitly described as "Built on rock-solid SQLite
  foundation," with Model classes declaring relations (`@children`,
  seen below).
- **Fully observable despite being SQLite-backed**: changing a record causes
  all UI depending on it to re-render automatically (list, counters,
  detail view, etc.) — contrasted explicitly with "using SQLite directly."
- **Offline-first**: ships a sync mechanism to a custom backend (detailed in
  the Sync unit).
- **Multiplatform**: iOS, Android, Windows, web, Node.js. Framework-agnostic
  JS API, with React bindings (`withObservables`) as the primary sugar.

## Usage shape (illustrative, from the README example)

Models are declared as classes with decorated fields/relations:

```js
class Post extends Model {
  @field('name') name
  @field('body') body
  @children('comments') comments
}
```

Components subscribe to a model or query via `withObservables(...)`, and
WatermelonDB propagates changes to any dependent component automatically —
"Doesn't matter if a change occurred in a totally different part of the
app, it all just works out of the box."

## Relevance to hoyadb's open questions

- WatermelonDB is the strongest existing precedent for pairing a
  **relational, SQLite-backed store** with a **fully reactive** app-facing
  layer — directly matching hoyadb's "relational-ish" + "subscription on
  entity/query" needs simultaneously, rather than treating them as
  competing concerns.
- Explicitly names "app must launch fast despite large local datasets" as
  the core performance goal, which is close in spirit to hoyadb's
  "must not feel slow" (not state-of-the-art) performance stance.
