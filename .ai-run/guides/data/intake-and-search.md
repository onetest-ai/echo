# Intake and Search

**Project**: Echo · **Scope**: Intake (what Echo reads from mail and channels, and whom it trusts) and the search index over the user's sources

**Reference**: `.ai-run/reference/data/recall-store.md` — how Intake and the index are built and reached (`ref: <section>`). It
changes with the code; these rules change only by a `Rules:` PR.

## Intake

### Rules
- R1. Intake and search stay off until the user turns them on, after an update too; while off nothing is read from mail or channels and nothing is indexed. The dictionary feeds the transcription hint whether they are on or off. Evidence: `brain::enabled`
- R2. Read mail and channel posts only as checked facts, each quote matched word for word against its message; never keep their text. Evidence: `brain::digest::check` (ref: Mail and channel posts)
- R3. Read a message only from the user, a colleague or an authenticated sender trusted for all its threads' clients; defer an author not yet looked up. Evidence: `brain::senders::Senders::mail`, `graph::mail_auth` (ref: Mail and channel posts)
- R4. Intake's facts are kept apart from the index, backed up, and never dropped by an update, a sync or a rebuild; their format changes only compatibly. They are removed only when their source is removed from the thread or the user deletes it. Evidence: `brain::sources` (ref: Intake facts)
- R5. A closed thread's sources and Intake facts are kept; only its index records go (R7). Evidence: `brain::store::archived_of`

## Search

### Rules
- R6. The search index is built only from the call folders, Intake's facts, thread documents and the dictionary; a rebuild or a schema change loses nothing. Evidence: `brain::store::open`
- R7. A record leaves the index when its source is gone (deleted or removed from its thread), when its thread is closed (its call card stays), or past its kind's horizon; leaving the index never deletes a source or an Intake fact. Evidence: `brain::store::remove` (ref: Retention)
- R8. Serve indexed data and Intake's facts to a session, job or plugin only through an access scoped to its profile; sources of other profiles answer "not found". Evidence: `brain::access::Access` (ref: Access)
- R9. The MCP server is read-only, and its tools are allowed only in a call session's ask mode, never in a minutes run. Evidence: `guard::BRAIN_TOOLS` (ref: Access)
- R10. Record ids stay the same across rebuilds. Evidence: `brain::chunk::chunk_id`
- R11. A change to how search ranks or what it returns keeps the made-up regression set passing, and its PR reports a blind evaluation at depth 5 against the current search on the held-out real queries kept on the developer's machine (never in the repo, testing R8). Evidence: `brain::store::search_in` tests (ref: Evaluation)
