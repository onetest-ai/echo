# Threads and Ledger — reference

How threads, clients, the work ledger, people and Microsoft 365 sends are built. It changes with the code, in the same
PR. The rules are in `.ai-run/guides/development/threads-and-ledger.md`.

## Proposals and evidence

- `ledger::apply` takes an agent's proposals. `ledger::quote_in` refuses a quote that is not one contiguous span
  (≥ 3 normalised words) of its evidence item; a status change also needs the verifier's `supports`.
- Closing a question or risk is op `answer`, verified; a near-duplicate is `suggest_merge`.
- The verify step runs in a fresh process, apart from the extractor (`projectsync::run`).

## The user's edits

`ledger::human_event` records the user's own edit: it pins the field, and an agent's later evidence lands in history
without changing it; each such edit can be undone.

## Threads and clients

- A thread (project) changes through `projects::update`: projects lock, load, `check`, write. A client through
  `projects::update_client`.
- A call's threads are computed from rules, pins and exclusions (`projects::calls_of`); `projects::suggest` only
  suggests.
- A link between two threads: `ledger::link_threads`, written in both ledgers.

## Mail and chat in a sync

`ledger::mail_sync` and `ledger::chat_sync` take mail and chat only from the user, colleagues and trusted,
authenticated senders.

## People

`people::decide` applies a suggestion after the quote check and the user's yes; `people::refresh_m365` turns a change to
a field or role the user set into a question.

## Microsoft 365 sends

- A command that sends, replies, posts or uploads returns `{sent: false, would}` without `--yes` (`m365::mail_send`).
- `m365::not_in_run` runs before a send: refused when `ECHOCTL_HEADLESS` is set, but for a routine's or Yolo's run acting
  on its own (`.ai-run/guides/security/untrusted-text.md` R21).
