# Threads and Ledger

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: threads, clients and the work ledger: evidence-checked proposals, user edits pinned, Microsoft 365 sends

## Threads, ledger and Microsoft 365 sends

### Rules
- R1. An agent's proposal to the work ledger counts only through `ledger::apply`, which refuses a quote that is not one contiguous span (≥ 3 normalised words) of its evidence item (`ledger::quote_in`); status changes also need the verifier's `supports`. Evidence: `ledger::quote_in`
- R2. Record the user's own edit through `ledger::human_event`: it pins the field, and an agent's later evidence lands in history without changing it; every such edit can be undone. Evidence: `ledger::human_event`
- R3. Change a thread (project) through `projects::update` (projects lock, load, `check`, write) and a client through `projects::update_client`. Evidence: `projects::update`
- R4. A Microsoft 365 command that sends, replies, posts or uploads returns `{sent: false, would}` without `--yes` and calls `m365::not_in_run` before it sends (refused when `ECHOCTL_HEADLESS` is set). Evidence: `m365::mail_send`
- R5. A call's threads are computed from rules, pins and exclusions, never stored in the call (a call can be in several; rules change), and a suggestion never assigns: a model or heuristic must not move a call into a thread. Evidence: `projects::calls_of`, `projects::suggest`
- R6. Colleagues never count toward a thread's `people` rules (they attend every client's calls); a client is an organisation marked `account`, partners and vendors are not. Evidence: `projects::CallFacts`, `projects::clients`
- R7. Closing a question or risk goes through op `answer`, verified, never a `description` edit; a near-duplicate is `suggest_merge`, never merged automatically. Evidence: `ledger::apply`
- R8. The verify step runs in a fresh process: the verifier must not share the extractor's context. Evidence: `projectsync::run`
- R9. Mail and chat reach a thread's sync only from the user, colleagues and trusted, authenticated senders: outside text reaching a tool-using run could carry instructions. Evidence: `ledger::mail_sync`, `ledger::chat_sync`
- R10. Nothing about a person changes without the user: people suggestions need the quote check and the user's yes; a field or role the user set is never overwritten (by Microsoft 365 or a proposal), it becomes a question. Evidence: `people::decide`, `people::refresh_m365`
- R11. A link between two threads is written in both ledgers and unlinks from both, so neither side's undo leaves half of it. Evidence: `ledger::link_threads`
