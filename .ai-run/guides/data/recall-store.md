# Recall Store

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: Recall SQLite store schema changes and profile-scoped access

**Design**: `.ai-run/guides/design/recall.md` — how it works and why; the rules here win.

## Recall store

### Rules
- R1. Change the Recall (Brain) schema in `brain::store::open` additively: `CREATE ... IF NOT EXISTS`, new columns added inside `BEGIN IMMEDIATE` after checking `pragma_table_info`, backfilled once (an older store and two processes opening it must both work). Evidence: `crates/echoctl/src/brain/store.rs::open`
- R2. Serve Recall data to a session or job through a `brain::access::Access` for its profile and scope; sources of other profiles answer "not found". Evidence: `crates/echoctl/src/brain/access.rs::Access`
