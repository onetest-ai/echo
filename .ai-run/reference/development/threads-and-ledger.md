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

## A plugin's writes

- `ledger::plugin_upsert` / `ledger::plugin_close` write a plugin's own tasks (events `by: plugin`, its id and key):
  a new one a `create` in the plugin's story (made by `plugin_story` on its first applied task), a change a `plugin`
  event `set`; `fold` applies a field unless the user pinned it, and only to the task of that plugin and key. With
  `propose` the event waits in `Tree::proposals` until the user's `accept` (`reject`: `Tree::refused`, the same again
  stays quiet).
- `projects::plugin_set_stage`: a `StageChange` with `by` and `by_name`; `projects::stage_held_from` says when the user
  overrode it, `Project::stage_handed` when they let it set the stage again (`projects::hand_stage_back`).

## Threads and clients

- A thread (project) changes through `projects::update`: projects lock, load, `check`, write. A client through
  `projects::update_client`.
- A call's threads are computed from rules, pins and exclusions (`projects::calls_of`); `projects::suggest` only
  suggests.
- A link between two threads: `ledger::link_threads`, written in both ledgers.

## What a sync uses

- Each pass (extract, verify, people) reads its usage from Claude Code's `result` event (`projectsync::usage_of`:
  `input`, `output`, `cache_read`, `cache_write`, `turns`, `cost_usd` when given) into its run's `log.txt` (one line a
  pass), the sync's state (`usage.<pass>`) and one JSON line a pass in `ledger/sync-usage.jsonl` (`at`, `run`, `pass`,
  `usage`).
- `echoctl project sync-stats [--days N]` (`projectsync::sync_stats`; default 7, today included) sums every thread's
  lines per day and pass: `{days: [{day, runs, tokens, passes: {<pass>: {runs, tokens, input, output, cache_read,
  cache_write, turns, cost_usd}}}], total}`, newest day first; `tokens` is the four token counts together.

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
