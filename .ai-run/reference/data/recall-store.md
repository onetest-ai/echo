# Recall Store — reference

How Intake's facts and the Recall (Brain) index are built and reached. It changes with the code, in the same PR. The
rules are in `.ai-run/guides/data/intake-and-search.md`.

## Schema changes

The schema is opened and migrated in `brain::store::open`: tables with `CREATE ... IF NOT EXISTS`; a new column is added
inside `BEGIN IMMEDIATE` after checking `pragma_table_info`, and backfilled once.

## Access

- A session or job reads Recall data through a `brain::access::Access` built for its profile and scope; a source of
  another profile answers "not found".
- The on/off switch: `brain::enabled`.
- The MCP server's tools are `mcp__echo__…`, listed in `guard::BRAIN_TOOLS`.

## Mail and channel posts

- Mail and posts are indexed as the quarantine reader's facts; `brain::digest::check` checks each quote word for word
  against its message.
- Who is read (`brain::senders::Senders::mail`, `graph::mail_auth`): the user; a colleague (Exchange
  `AuthAs: Internal`, a Teams author by directory id); a sender trusted for all its threads' clients, when outside mail
  passes DMARC or compauth. An author not yet looked up is deferred.

## Intake facts

- One JSON file per mail conversation or channel post, `<home>/recall/intake/facts/<source>.json`, by the index's
  source id: `mail/<short>` → `facts/mail/<short>.json`, `channel/<short>/<post>` → `facts/channel/<short>/<post>.json`
  (`brain::facts::path_of`; an id that isn't plain parts gets no file).
- A file (`brain::facts::Facts`, `version` 1; a newer field always has a default): `source`, `kind` (mail | channel),
  `title`, `event_date`, `refs` (`threads`, `client`, `people`, `url`, `subjects`, `subject`, a mail's `conversation`
  and `message` ids, a post's `channel` and `channel_name`), `sections` (`head` "YYYY-MM-DD HH:MM <who>", `from`,
  `at`, `link`, `facts`: `[{text, quote?}]`) and `archived` (every thread it is under is closed).
- A sync (`brain::sources::sync`) writes each read source's file first, written atomically, then feeds the index from it
  (`Facts::input`); it then puts every file under the threads that match it now (`brain::sources::place`): moved, the
  file and the index rewritten; matched by none (its source left its thread), the file and its index rows removed.
- A sender trusted or ignored (`request_reread`) reads its clients' sources again: their facts within the 90-day
  reading window go and are read again; older ones stay. A new reader version reads every window again and drops
  nothing.
- Readers: `share::source` / `share::sources` / `sources_changed` (`mail_item`, `mail_messages`) and `links::mail_text`
  / `links::mail_url` read the files; the sections they give are the index's (`Facts::pieces`, the same chunker).
- The move from the index (`brain::facts::migrate`): at the app's start (`brain::start_scheduler`) and before every
  sync, under `intake/.migrate.lock`, until `intake/migrated.json` records it done. It reads `knowledge.sqlite`
  read-only, writes a file for every mail and channel document that has none, each only when its facts give the index's
  sections back text for text, and marks it done only when every document has its file.

## Intake facts backup

- Before a sync's first change to the facts (a write that changes a file, or a removal), `brain::facts::backup` copies
  `facts/` to `intake/facts.bak/`: one rolling copy, made whole beside it (`.facts.bak.tmp`) and then put in place of
  the one before. Snapshots of `knowledge.sqlite` don't cover the facts.

## Ids, archive, snapshots

- Chunk ids: `brain::chunk::chunk_id`.
- A closed thread's sources are archived: `brain::store::archived_of`.
- A sync that changes the store snapshots it first: `brain::store::snapshot`.
