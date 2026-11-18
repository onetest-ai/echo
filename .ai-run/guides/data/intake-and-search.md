# Intake and Search

**Project**: Echo · **Scope**: Intake (what Echo reads from mail and channels, and whom it trusts) and the search index over the user's sources

**Reference**: `.ai-run/reference/data/recall-store.md` — how Intake and the index are built and reached (`ref: <section>`). It
changes with the code; these rules change only by a `Rules:` PR.

## Intake

### Rules
- R1. Intake stays off until the user turns it on, after an update too; while off nothing is read or indexed. The dictionary feeds the transcription hint whether Intake is on or off. Evidence: `brain::enabled`
- R2. Read mail and channel posts only as checked facts, each quote matched word for word against its message; never keep their text. Evidence: `brain::digest::check` (ref: Mail and channel posts)
- R3. Read a message only from the user, a colleague or an authenticated sender trusted for all its threads' clients; defer an author not yet looked up. Evidence: `brain::senders::Senders::mail`, `graph::mail_auth` (ref: Mail and channel posts)
- R4. Intake's facts are the only copy of what was read (a message past the reading window can't be read again): change their format only compatibly, back them up, and never drop them on an update or a sync. Evidence: `brain::sources`
- R5. A closed thread's sources are kept, never dropped. Evidence: `brain::store::archived_of`

## Search

### Rules
- R6. The search index is derived from the call folders, Intake's facts, thread documents and the dictionary: it can be rebuilt at any time, a schema change rebuilds it, and nothing is lost when it is. Evidence: `brain::store::open`
- R7. The index is working memory, not an archive: a record leaves it when its source is gone, its thread is closed (but its call card stays) or it is past its kind's horizon; leaving the index never deletes the source. Evidence: `brain::store::remove` (ref: Retention)
- R8. Serve search results to a session or job only through an access scoped to its profile; sources of other profiles answer "not found". Evidence: `brain::access::Access` (ref: Access)
- R9. The MCP server is read-only, and its tools are allowed only in a call session's ask mode, never in a minutes run. Evidence: `guard::BRAIN_TOOLS` (ref: Access)
- R10. Keep record ids stable across rebuilds, so a link to a moment keeps working. Evidence: `brain::chunk::chunk_id`
- R11. A change to how search ranks or what it returns ships only when it wins a blind evaluation at a fixed depth on held-out real queries, with a made-up regression set in the repo. Evidence: `tests/` (ref: Evaluation)
