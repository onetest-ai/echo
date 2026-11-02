# Dependency pass — the whole tree in view

Extraction looks at one item and the nodes in front of it, so it misses dependencies **across
stories** (the largest gap in testing). Read the same items again with the
whole tree in view and propose only two ops (schema and the quote rule as in EXTRACT.md):

- `link` — `node` can't finish (or start) until `depends_on` does. Both existing stories or tasks,
  anywhere in the tree. Typical: a document waits on the sessions that feed it; a workshop waits on
  findings, interviews or recruiting; access waits on a request being sent; a revised plan waits on
  leadership alignment.
- `waiting` — `node` is held up by a person's action or availability that isn't itself a node:
  replies, calendars, sign-offs, someone being away. Give `who` and `what`.

Not a dependency: a plain to-do, a wish with no work waiting on it, two unrelated things in
sequence. Don't link a story to its own tasks. Skip anything already in `waits_for` or
`waiting_for`.

1. `$L context` once.
2. For each evidence id: read its `.md`, write `<run folder>/<id>_deps.json`, run
   `$L apply --evidence <id> --proposals <that file>`, fix rejections in `<id>_deps_fix.json`.
   A node created after the item's date can't be linked from it: skip it.
