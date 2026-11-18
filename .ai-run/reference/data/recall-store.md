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
  (`brain::facts::path_of`; an id that isn't plain parts gets no file: a sync leaves it out and says so,
  `brain::sources::keep`).
- A file (`brain::facts::Facts`, `version` 1; a newer field always has a default): `source`, `kind` (mail | channel),
  `title`, `event_date`, `refs` (`threads`, `client`, `people`, `url`, `subjects`, `subject`, a mail's `conversation`
  and `message` ids, a post's `channel` and `channel_name`), `sections`, `archived` (every thread it is under is closed)
  and `migrated_raw` (moved from the index as its text was). A section: `head` "YYYY-MM-DD HH:MM <who>", `from`, `at`
  (RFC 3339), `link`, `facts` (`[{text, quote?}]`), `sender` (as Who may speak names them: a mail's address, a Teams
  author's id), `unverified` (read unauthenticated after "Read anyway") and `raw` (its facts are the index's text).
- A sync (`brain::sources::sync`) writes each read source into its file first, merged into what the file held
  (`brain::facts::merge`: a section is the same message by its heading and link; one read again replaces it, a new
  one is added, one the new read lacks stays; people and subjects of both reads; the later date), then feeds the index
  from it (`Facts::input`).
- It then puts every file under the threads that match it now (`brain::sources::place`): moved, the file and the index
  rewritten; matched by none (its source left its thread), the file and its index rows removed. This runs only when
  every thread was read (`projects::list_whole`: none unreadable, at least one); otherwise nothing is moved or removed
  that sync and the reason is logged. More than a fifth of the files leaving at once (past 5) is refused and logged.
- Who may speak (`brain::senders::decide`): Trust and Read anyway read the client's sources again
  (`request_reread`), merged in, nothing taken out; Ignore and Keep out ask (`request_drop`, `intake/drops.json`)
  for that sender's sections to leave that client's facts at the next sync (`apply_drops`; Keep out: only their
  `unverified` ones; a section from before `sender` was kept: by its author's name); a source left with no section goes.
- A new reader version reads every window again and merges; it drops nothing.
- Readers: `share::source` / `share::sources` / `sources_changed` (`mail_item`, `mail_messages`) and `links::mail_text`
  / `links::mail_url` read the files; the sections they give are the index's (`Facts::pieces`, the same chunker).
- The move from the index (`brain::facts::migrate`): at the app's start (`brain::start_scheduler`, taking the sync's
  lock `brain.lock`, skipped while a sync holds it) and at the start of every sync (`migrate_locked`, under that lock),
  until `intake/migrated.json` records it done with its counts (`documents`, `written`, `kept`, `raw`, `skipped`). It
  reads `knowledge.sqlite` read-only. A document that has a file keeps it; one whose facts give the index's sections
  back text for text is written from them, else from its sections' text (`migrated_raw`, its sections `raw`); one no
  file can be named for is listed as skipped with why. It is marked done once every document is accounted for.
- With Intake off the dictionary's view (`taxonomy::view_in`) has no index counts and never opens or makes the index:
  the transcription hint (`projects::terms_for_meeting`) reads the dictionary alone.

## Intake facts backup

- Before a sync's first change to the facts (a write that changes a file, or a removal), `brain::facts::backup` copies
  `facts/` into today's generation, `intake/facts.bak/YYYY-MM-DD/`, made whole under a name of its own first
  (`.copy.<pid>.<n>.tmp`) and then renamed into place.
- A copy with less than half the files of today's generation never replaces it: it is kept beside it as
  `YYYY-MM-DD-HHMMSSmmm`.
- The generations of the newest 7 days are kept, and always the generation with the most files. Snapshots of
  `knowledge.sqlite` don't cover the facts.

## Ids, archive, snapshots

- Chunk ids: `brain::chunk::chunk_id`.
- A closed thread's sources are archived: `brain::store::archived_of`.
- A sync that changes the store snapshots it first: `brain::store::snapshot`.
