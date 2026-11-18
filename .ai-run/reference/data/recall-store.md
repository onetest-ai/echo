# Recall Store — reference

How the Recall (Brain) store is built and reached. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/data/recall-store.md`.

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

## Ids, archive, snapshots

- Chunk ids: `brain::chunk::chunk_id`.
- A closed thread's sources are archived: `brain::store::archived_of`.
- A sync that changes the store snapshots it first: `brain::store::snapshot`.

## Search

- `brain::store::search_at`: the candidates are the keyword (FTS5, BM25) top 30 after scope, access and history;
  the query's words skip `STOPWORDS` (English, Ukrainian, Russian).
- With an embedding server that answers, the candidates are re-ranked by `brain::store::rerank`: weighted reciprocal
  rank (k = 60) of the keyword rank (0.4) and the vector rank among the candidates (0.6). No section outside the
  candidates is added; without embeddings the keyword order stays.
- A closed thread's sections weigh `ARCHIVE_WEIGHT` against current ones, except within that thread.
- The keyword text of a section (`brain::store::fts_text`) leaves out the minutes' `Who:`, `Thread:`, `Threads:` and
  `Client:` lines that carry echo:// links, and reduces other echo:// links to their labels. A store whose
  `meta.fts_text` differs from `FTS_TEXT_VERSION` has it written again from `chunks` at `open`.

## Evaluation

- The made-up regression set: `brain::store::tests::the_made_up_regression_set_finds_its_sources_first`.
- The blind evaluation runs the built `echoctl brain search` on a copy of the developer's store (`ECHOCTL_HOME` and
  `ECHOCTL_CONFIG_DIR` pointing into a temp folder), at depth 5, graded 0/1/2 per unit (0: unrelated, 1: related,
  2: answers); it reports nDCG@5, P@5 and how many queries got an answer. The queries and grades stay off the repo.
