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
- The MCP server's tools are `mcp__echo__…`, listed in `guard::BRAIN_TOOLS`: `search_sources`, `list_sources`,
  `read_source`, `read_link` and `list_scopes` (every client and thread the reader may see, with its id; the session
  block names at most 8 of each) (`brain::mcp::tools`; the old names `search_knowledge` and `read_document` still
  answer). Who people are and how things relate are the Knowledge plugin's: no people or relations tools.
- What a session is told Echo holds (`brain::advert::summary`, the MCP server's instructions and `<echo_recall>`) and
  the window's per-view hint (`view::view_lines`) name `search_sources` and point to the Knowledge plugin for people.

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
  author's id), `unverified` (read unauthenticated after "Read anyway"), `id` (the message's Graph id), `internet_id` (a
  mail's `internetMessageId`, which stays when the mail moves folders) and `raw` (its facts are the index's text).
- A file that can't be read is moved to `intake/facts.broken/<source>.<time>.json` (out of `facts/` and its backups)
  before a new one is written (`Writer::write`).
- A sync (`brain::sources::sync`) reads nothing until the move from the index is done (`facts::migrated`); why it is
  paused, and the files it couldn't write in its last sync, are in `intake/status.json` (`facts::status`), shown on
  Settings → Intake's Last read line. A file that can't be written is left out of that sync and said; a drop request
  one of its files couldn't take stays for the next sync.
- It writes each read source into its file first, merged into what the file held (`brain::facts::merge`: a section is
  the same message by a mail's `internetMessageId`, else its Graph id, or, when one of the two has neither, by its
  heading and link; one read again replaces
  it, each old section at most once, so two sections of one read are never folded into one; a new one is added; one
  the new read lacks stays; people and subjects of both reads; the later date), then feeds the index from it
  (`Facts::input`), with every fact file the index has no document of (`missing_from_index`).
- It then puts every file under the threads that match it now (`brain::sources::place`): moved, the file and the index
  rewritten; matched by none (its source left its thread), the file and its index rows removed. This runs only when
  every thread was read (`projects::list_whole`: none unreadable, at least one); otherwise nothing is moved or removed
  that sync and the reason is logged. More than a fifth of the files leaving at once (past 5) is refused and logged.
- Who may speak (`brain::senders::decide`): Trust and Read anyway read the client's sources again
  (`request_reread`, `recall/reread.json`), merged in, nothing taken out; Ignore and Keep out ask (`request_drop`,
  `intake/drops.json`) for that sender's sections to leave that client's facts at the next sync (`apply_drops`; Keep
  out: only their `unverified` ones). Only sections that recorded that sender's id go; a section with only the
  author's name stays, and how many stayed is logged. A source left with no section goes.
- The request files are written under their lock (`<name>.lock`). A sync takes them by renaming them to
  `<name>.processing.json` under that lock (adding to one a sync before didn't finish), and deletes that file only once
  the requests were applied; a request made meanwhile waits in a new file for the next sync. A request file that can't
  be read is renamed `<name>.broken-<time>`, never read as empty or written over.
- A new reader version reads every window again and merges; it drops nothing.
- Readers: `share::source` / `share::sources` / `sources_changed` (`mail_item`, `mail_messages`) and `links::mail_text`
  / `links::mail_url` read the files; the sections they give are the index's (`Facts::pieces`, the same chunker).
- The move from the index (`brain::facts::migrate`): at the app's start (`brain::start_scheduler`, taking the sync's
  lock `brain.lock`, skipped while a sync holds it) and at the start of every sync (`migrate_locked`, under that lock),
  until `intake/migrated.json` records it done with its counts (`documents`, `written`, `kept`, `raw`, `skipped`). It
  reads `knowledge.sqlite` read-only. A document that has a file keeps it; one whose facts give the index's sections
  back text for text is written from them, else from its sections' text (`migrated_raw`, its sections `raw`); one no
  file can be named for is listed as skipped with why. A failed write stops it unmarked, to run again. It is marked
  done once every document is accounted for.
- With Intake off the dictionary's view (`taxonomy::view_in`) has no index counts and never opens or makes the index:
  the transcription hint (`projects::terms_for_meeting`) reads the dictionary alone.

## Intake facts backup

- Before a sync's first change to the facts (a write that changes a file, or a removal), `brain::facts::backup` copies
  `facts/` into today's generation, `intake/facts.bak/YYYY-MM-DD/`, made whole under a name of its own first
  (`.copy.<pid>.<n>.tmp`). The generation it replaces is renamed aside (`.old.<pid>.<n>.tmp`), the copy renamed in,
  and only then the old one deleted; what a backup stopped half way left (`.*.tmp`) is deleted at the next one.
- A copy with fewer files than today's generation never replaces it: it is the day's one shrunk copy,
  `YYYY-MM-DD-shrunk`, replacing an earlier shrunk copy of that day.
- The generations of the newest 7 days are kept, and always the generation with the most files. Snapshots of
  `knowledge.sqlite` don't cover the facts.

## Ids, archive

- Chunk ids: `brain::chunk::chunk_id`. A document's card is `chunk_id(source, store::CARD_ORD)`.
- A closed thread's sources are archived: `brain::store::archived_of`. What of them stays in the index is in
  Retention.
- The index has no snapshots (it is rebuilt from its sources) and no `related` table (dropped at `open`). At the app's
  start `brain::drop_old_folders` removes `recall/snapshots/` and `recall/assets/` (the old page images) when they are
  there; nothing else under `recall/` is touched.

## Thread folders

- `brain::files::folder::sync` reads each file's text: md, html, txt, transcripts, docx, xlsx, pptx and PDF (its text
  layer, by PDFium), each page or slide with its tables; nothing is read from page images (a scan or a picture-only
  slide is the Knowledge plugin's). `READER_VERSION` 3 reads every file again once, from its text.

## Retention

The index is working memory, not an archive (`brain::retain`). Each section has a kind, `chunks.section`, given by
`brain::store::section_kind` when the section is written. A store from before the column gets the kinds once, at
`open`. Mail and channel facts are dated by their own message once per `FACT_DAYS_VERSION` (`meta.fact_days`), at
`open`, by one writer.

| Section kinds | Kept (`retain::horizon`) |
|---|---|
| `transcript`, `chat` | `TALK_DAYS` (45) after the call's day |
| `summary`, `decisions`, `action_items`, `open_questions`, `minutes` (any other minutes section) | `MINUTES_DAYS` (90) |
| `mail`, `channel` | `FACTS_DAYS` (90) after the message's own day |
| `document` | while one of its threads is open |
| `card` | always |

- **Days.** Each section is dated in local days, as "today" is.
  - A call's sections take the call's day.
  - A mail or channel fact takes its own message's day: its heading's time (Microsoft 365's UTC) as a local day
    (`store::fact_day`). The search's `since` and `until` compare with it too.
- **Call cards.** A call's card is the last section of its document, under `## Call card` (`store::CALL_CARD`).
  - It holds the call's title and day, its threads, its people (the speakers, then the invite's people, at most 12)
    and its summary's first line.
  - Appending it leaves every other section's position and id, so unchanged sections keep their vectors.
- **Closed threads.** A section whose threads are all closed leaves the index; its call's card stays.
  - A closed thread's document gets a card in place of its text: one section at `store::CARD_ORD` with its title,
    path, threads and headings (at most 40).
- **When a document is written** (`store::sync_kept` with a `store::Keep`: today and the closed threads), the
  sections past their horizon or only in closed threads are left out. They are never written nor embedded; a card
  always is.
  - A document's own sections are the exception: the sweep replaces them with its card.
  - The document's hash is its text's, so a section reaching its horizon later never makes it read again.
  - A document written without some sections for its closed threads is marked in `documents.retained`
    (`closed:<thread ids>`).
- **The sweep** (`retain::apply`) takes what aged since a section was written, and closed threads' sections.
  - It runs at the end of `brain::sync` when `retain::due` says so: once a day, and when the set of closed threads
    changed (`meta.retained_for`).
  - The sweep waits while Echo records a call, or a call is being transcribed: the recorder's state, or a
    `.transcribing` marker (`transcribe::IN_PROGRESS`) whose process is alive (`brain::recording_or_transcribing`).
    Battery doesn't hold it back.
    - The sync itself still runs; its report says `sweep_waits`.
    - The app's next pass, a minute later, tries again.
  - It reads every section and every section's threads once. It deletes index rows only and marks documents as
    above. The report's `retired` counts the sections that left.
- **Restore.** `retain::restore`, at the start of every sync, drops a marked document whose closed threads changed,
  so the same sync reads it back with the same ids.
  - A call comes back from its folder.
  - A file comes back from its folder; its registry's `indexed_sha` is cleared.
  - Mail and posts come back from their Intake fact files (`sources::feed_missing`). That runs at every sync, with or
    without a Microsoft 365 sign-in, before the move is done, and when the thread list can't be read.
- The call folders, the thread folders' files, the dictionary and Intake's facts are never touched.
- The app's sync runs at least once a day and when a thread closes or opens again (`brain::fingerprint`).

## Search

- `brain::store::search_at`: the candidates are the keyword (FTS5, BM25) top 30 (or the limit asked, when higher),
  read best first until that many pass scope, access, history and the filters; the query's words skip `STOPWORDS`
  (English, Ukrainian, Russian). A query of only stopwords searches nothing and says so in `note`.
- Filters (`store::Filters`; `echoctl brain search --kind/--section/--since/--until/--person`; search_sources'
  `kind`, `section`, `since`, `until`, `person`) are applied before the cut:
  - source kinds `call`, `mail`, `channel`, `document`, by the source id's prefix (`store::SOURCE_KINDS`), in SQL;
  - section kinds (`store::SECTION_KINDS`, as in Retention), in SQL;
  - `since` and `until`, days compared with the section's day (Retention, Days), in SQL;
  - `person`, a part of the section's speaker: a transcript turn's names or a fact's author. It is compared in Rust,
    in lowercase in any script, with `%` and `_` taken as plain letters. Minutes and documents have no speaker.
- Unknown kinds and malformed days are refused. The answer's `filters` gives what was applied.
- With neither `since` nor `until`, a date in the query (`brain::when::in_query`) becomes the filter, and its words
  leave the query; `filters.dates_from_query` says which words.
  - Explicit dates: an ISO day, or a day with its month in English, Ukrainian or Russian (any form), with or without
    a year. A day without a year is the latest such day not after today.
  - An English month word counts only when it is capitalised, or in a form only a date has: with a year
    ("3 may 2026") or a day and a comma ("may 3, …"). So "step 3 may fail" is no date.
  - Relative dates: today, yesterday, this week, last week (Monday to Sunday), the past 7 days and last month, all
    in local days.
  - A month or a number alone is never a date. Several dates span from the first to the last.
- **Listing days.** Days are asked for, and no section of them has the query's words, or the query has none ("what
  happened yesterday"). The answer then lists those days' sources, newest first, each once (a call by its card).
  - Its `note` says so: `LISTED` or `LISTED_NO_MATCH`.
  - It is not re-ranked.
- A range that reaches before the shortest horizon of the section kinds it may find adds `retain::OLDER` to `note`:
  "older than the index keeps: see the call folders or Knowledge".
- Each hit has its `section_kind`.
- With an embedding server that answers, the candidates are re-ranked by `brain::store::rerank`: weighted reciprocal
  rank (k = 60) of the keyword rank (0.4) and the vector rank among the candidates (0.6). No section outside the
  candidates is added; without embeddings the keyword order stays.
- A closed thread's sections weigh `ARCHIVE_WEIGHT` against current ones, except within that thread.
- The keyword text of a section (`brain::store::fts_text`) leaves out of the minutes' `Who:`, `Thread:`, `Threads:`
  and `Client:` lines that carry echo:// links their label and links (what else such a line says stays), and reduces
  other echo:// links to their labels. A store whose `meta.fts_text` differs from `FTS_TEXT_VERSION` has it written
  again from `chunks` at `open`, in batches of `FTS_BATCH` sections (`meta.fts_text_next`), each only when no one else
  writes, for `FTS_OPEN_BUDGET` per open; busy or out of time, the rest waits for a later open. Done, rows of
  `chunks_fts` without a section are dropped.
- Term mentions after a dictionary edit (`brain::taxonomy::mentions::mentions_of`) take their candidates from the
  keyword index and from the sections with an echo:// link (read once per edit), so a name only in a header line
  counts as in a full scan.

## Evaluation

- The made-up regression set: `brain::store::tests::the_made_up_regression_set_finds_its_sources_first`.
- The blind evaluation runs the built `echoctl brain search` on a copy of the developer's store (`ECHOCTL_HOME` and
  `ECHOCTL_CONFIG_DIR` pointing into a temp folder), at depth 5, graded 0/1/2 per unit (0: unrelated, 1: related,
  2: answers); it reports nDCG@5, P@5 and how many queries got an answer. The queries and grades stay off the repo.
