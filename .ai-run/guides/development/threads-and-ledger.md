# Threads and Ledger

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: threads, clients and the work ledger: evidence-checked proposals, user edits pinned, Microsoft 365 sends

**Design**: `.ai-run/guides/design/threads.md` — how it works and why; the rules here win.

## Threads, ledger and Microsoft 365 sends

### Rules
- R1. An agent's proposal to the work ledger counts only through `ledger::apply`, which refuses a quote that is not one contiguous span (≥ 3 normalised words) of its evidence item (`ledger::quote_in`); status changes also need the verifier's `supports`. Evidence: `crates/echoctl/src/ledger.rs::quote_in`
- R2. Record the user's own edit through `ledger::human_event`: it pins the field, and an agent's later evidence lands in history without changing it; every such edit can be undone. Evidence: `crates/echoctl/src/ledger.rs::human_event`
- R3. Change a thread (project) through `projects::update` (projects lock, load, `check`, write) and a client through `projects::update_client`. Evidence: `crates/echoctl/src/projects.rs::update`
- R4. A Microsoft 365 command that sends, replies, posts or uploads returns `{sent: false, would}` without `--yes` and calls `m365::not_in_run` before it sends (refused when `ECHOCTL_HEADLESS` is set). Evidence: `crates/echoctl/src/m365.rs::mail_send`
