# Recall Store

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: Recall SQLite store schema changes and profile-scoped access

## Recall store

### Rules
- R1. Change the Recall (Brain) schema in `brain::store::open` additively: `CREATE ... IF NOT EXISTS`, new columns added inside `BEGIN IMMEDIATE` after checking `pragma_table_info`, backfilled once. Evidence: `brain::store::open`
- R2. Serve Recall data to a session or job through a `brain::access::Access` for its profile and scope; sources of other profiles answer "not found". Evidence: `brain::access::Access`
- R3. Recall stays off until the user turns it on, after an update too; while off nothing is read, indexed or tagged. Evidence: `brain::enabled`
- R4. Index mail and channel posts only as the quarantine reader's facts, each quote checked word for word against its message; never their text. Evidence: `brain::digest::check`
- R5. Read a message only from the user, a colleague (Exchange `AuthAs: Internal`, or a Teams author by directory id, never by name) or a sender trusted for every client of its threads; trusted outside mail also needs DMARC or compauth passing for the From domain; an author not looked up yet defers the document. Evidence: `brain::senders::Senders::mail`, `graph::mail_auth`
- R6. The MCP server is read-only, and its tools (`mcp__echo__…`) are allowed only in a call session's ask mode, never in a minutes run. Evidence: `guard::BRAIN_TOOLS`
- R7. Keep chunk ids stable across syncs. Evidence: `brain::chunk::chunk_id`
- R8. Old MCP tool names keep answering. Evidence: `brain::mcp::canonical`
