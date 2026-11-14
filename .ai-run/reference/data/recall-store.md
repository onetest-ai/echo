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

## Learned vocabulary in config

Every saved config writes `learned` (`[]` when empty) (`config::learned_always`); `vocab::migrate` migrates only a
config without the key.
