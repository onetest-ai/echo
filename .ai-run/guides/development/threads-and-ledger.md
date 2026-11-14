# Threads and Ledger

**Project**: Echo · **Scope**: threads, clients and the work ledger: evidence-checked proposals, user edits pinned, Microsoft 365 sends

**Reference**: `.ai-run/reference/development/threads-and-ledger.md` — how the ledger, threads, people and sends are
built (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Threads, ledger and Microsoft 365 sends

### Rules
- R1. An agent's proposal to the work ledger counts only through `ledger::apply`, each change quoting its evidence word for word; a status change also needs the verifier's support. Evidence: `ledger::quote_in` (ref: Proposals and evidence)
- R2. The user's own edit pins its field: an agent's later evidence lands in history without changing it, and every such edit can be undone. Evidence: `ledger::human_event` (ref: The user's edits)
- R3. Change a thread (project) or a client only through its update helper. Evidence: `projects::update` (ref: Threads and clients)
- R4. A Microsoft 365 send, reply, post or upload happens only with the user's explicit yes and never from a run, but for a routine's or Yolo's run acting on its own (`untrusted-text.md` R21). Evidence: `m365::mail_send` (ref: Microsoft 365 sends)
- R5. A call's threads are computed from rules, pins and exclusions, never stored in the call; a suggestion never assigns a call to a thread. Evidence: `projects::calls_of`, `projects::suggest`
- R6. Closing a question or risk is an answer, verified, never a description edit; a near-duplicate is suggested for merging, never merged automatically. Evidence: `ledger::apply` (ref: Proposals and evidence)
- R7. The verify step runs in a fresh process, apart from the extractor. Evidence: `projectsync::run`
- R8. Mail and chat reach a thread's sync only from the user, colleagues and trusted, authenticated senders. Evidence: `ledger::mail_sync`, `ledger::chat_sync`
- R9. Nothing about a person changes without the user: suggestions need the quote check and the user's yes; a field or role the user set is never overwritten (by Microsoft 365 or a proposal), it becomes a question. Evidence: `people::decide`, `people::refresh_m365`
- R10. A link between two threads is written in both ledgers and unlinks from both. Evidence: `ledger::link_threads`
