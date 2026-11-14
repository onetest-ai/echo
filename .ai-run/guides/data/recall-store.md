# Recall Store

**Project**: Echo · **Scope**: Recall SQLite store schema changes and profile-scoped access

**Reference**: `.ai-run/reference/data/recall-store.md` — how the store is built and reached (`ref: <section>`). It
changes with the code; these rules change only by a `Rules:` PR.

## Recall store

### Rules
- R1. Change the Recall (Brain) schema only additively, where the store is opened: a new column added once, in a transaction, and backfilled once. Evidence: `brain::store::open` (ref: Schema changes)
- R2. Serve Recall data to a session or job only through an access scoped to its profile; sources of other profiles answer "not found". Evidence: `brain::access::Access` (ref: Access)
- R3. Recall stays off until the user turns it on, after an update too; while off nothing is read, indexed or tagged. Evidence: `brain::enabled`
- R4. Index mail and channel posts only as checked facts, each quote matched word for word against its message; never their text. Evidence: `brain::digest::check` (ref: Mail and channel posts)
- R5. Read a message only from the user, a colleague or an authenticated sender trusted for all its threads' clients; defer an author not yet looked up. Evidence: `brain::senders::Senders::mail`, `graph::mail_auth` (ref: Mail and channel posts)
- R6. The MCP server is read-only, and its tools are allowed only in a call session's ask mode, never in a minutes run. Evidence: `guard::BRAIN_TOOLS` (ref: Access)
- R7. Keep chunk ids stable across syncs. Evidence: `brain::chunk::chunk_id`
- R9. A closed thread's sources are archived, never dropped from the store. Evidence: `brain::store::archived_of`
- R10. A sync that changes the store snapshots it first. Evidence: `brain::store::snapshot`
