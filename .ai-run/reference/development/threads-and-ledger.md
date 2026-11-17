# Threads and Ledger — reference

How threads, clients, the work ledger, people and Microsoft 365 sends are built. It changes with the code, in the same
PR. The rules are in `.ai-run/guides/development/threads-and-ledger.md`.

## Proposals and evidence

- `ledger::apply` takes an agent's proposals. `ledger::quote_in` refuses a quote that is not one contiguous span
  (≥ 3 normalised words) of its evidence item; a status change also needs the verifier's `supports`.
- Closing a question or risk is op `answer`, verified; a near-duplicate is `suggest_merge`.
- The verify step runs in a fresh process, apart from the extractor (`projectsync::run`).

## A sync's runs

`projectsync::run` starts its Claude Code runs on the thread's AI profile and model:

- **extract** reads the work and who's who in one run: the items not read yet (at most `MAX_ITEMS`, 10), then items
  read for the work already but not yet for people (`ledger/people-read-2.json`), oldest first
  (`projectsync::take_items`). Its message stays within `MAX_TEXT` (200,000 characters, counted as characters): the
  thread's `context` and `people-context` (`projectsync::Given`) first, the items' text in what is left (at least
  `MIN_ROOM`, 20,000); an item is cut at `MAX_ITEM` (120,000) with "[N characters left out]" at its end, the first item
  always goes (cut to fit), the rest whole or at the next sync. The text is read as bytes, invalid UTF-8 replaced. An
  item cut, and one whose `.md` is missing (marked processed, "Echo couldn't read its text"; marked read for people),
  is said in the sync's message; one that can't be read for another reason waits for the next sync.
- Its fixed instructions — how the run works, then the project-sync skill's `EXTRACT.md` and `PEOPLE.md`, compiled in —
  go in `--append-system-prompt-file`, the same for every run and thread (`projectsync::extract_rules`); its one message
  — the thread, the run's folder, the items in order, the run's evidence tag, `context`, `people-context` (without its
  file list), then each item's `pending` entry and text in a block `<evidence-<12 hex> id="…">` … `</evidence-<12 hex>>`,
  the tag random for each run (`projectsync::evidence_tag`) — on stdin (`projectsync::extract_pass`).
- Tools `Bash,Write`: it writes `<id>.json` and `<id>_people.json` (`[]` when none) in its run folder and runs `apply`,
  `people-apply` and `done`. `apply --done '<note>'` marks the item processed in the same step only when nothing was
  rejected (else `done: null`, `not_done`: fixed first, then `done`); `done` and `--done` only for the run's own items for
  the work (`ECHOCTL_RUN_ITEMS`). `people-apply` in a run notes the item in the run folder's `people-applied.txt`; once
  the run ends Echo marks read for people only the items noted there or marked processed during the run
  (`projectsync::read_for_people`); an item offered `PEOPLE_OFFERS` (2) times without is marked read anyway, said in
  the run's log and the sync's message (`ledger/people-offered.json`, `projectsync::people_handled`).
- **verify** runs when `verify_prep` has packets: no tools, one request a batch of packets within `MAX_PACKETS` (60,000
  characters of their JSON; each batch a fresh run, their usage summed in the state's `usage.verify`); its fixed
  instructions are `VERIFY.md` and the answer's form, its message the packets with `me_patterns` and `thread_scope`
  (`projectsync::verify_brief`, which `verify-prep --out` prints too). Echo reads the answer's verdicts (a fenced JSON
  block first, else the first JSON array of entries with a `mid`: `projectsync::verdicts_of`) and applies only those on
  its own packets through `ledger::verify_apply`; an answer it can't read leaves its packets for the next sync, and a
  verify run that fails leaves its packets and the later batches' (`projectsync::verify`): the sync still ends done,
  with the extract's report. The run
  folder keeps `packets.json` and `verdicts.json`; the sync's message gets "Verify: N supports, …".
- The skill (`plugin/skills/project-sync/SKILL.md`) runs the same instructions by hand, with the ledger's CLI steps
  (`pending`, `context`, `people-context`, `apply`, `people-apply`, `done`, `verify-prep`, `verify-apply`).

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

- Each pass (extract, verify) reads its usage from Claude Code's `result` event (`projectsync::usage_of`:
  `input`, `output`, `cache_read`, `cache_write`, `turns`, `cost_usd` when given) into its run's `log.txt` (one line a
  pass), the sync's state (`usage.<pass>`, with `ok`, and `error` when it failed; a failed sync's state keeps it too)
  and one JSON line a pass in `ledger/sync-usage.jsonl` (`at`, `run`, `pass`, `usage`, `ok`, `error`), written in one
  write. A failed pass keeps what it used when its result event came (`projectsync::RunFailed`: an error result, its
  turns used up among them); stopped with none (its time limit, Echo quitting) its `usage` is null: "usage unknown". A failure before Claude
  Code started (too old for the guard, refused, not started) is no run and isn't counted.
- `echoctl project sync-stats [--days N]` (`projectsync::sync_stats`; default 7, today included, by calendar day)
  sums every thread's lines per day and pass: `{days: [{day, runs, tokens, passes: {<pass>: {runs, failed, tokens,
  input, output, cache_read, cache_write, turns, cost_usd, cost_runs}}}], total}`, newest day first; `tokens` is the
  four token counts together; a run of unknown usage counts as a run with no tokens; `cost_usd` sums only the
  `cost_runs` that gave a cost. Only the threads there now: a deleted thread's usage leaves the stats with it.

## Mail and chat in a sync

`ledger::mail_sync` and `ledger::chat_sync` take mail and chat only from the user, colleagues and trusted,
authenticated senders.

- The app's scheduler (`projectsync::start_scheduler`, every 5 minutes but from 22:30 to 07:00, `projectsync::quiet`;
  `echoctl project ledger <id> run --scheduled`,
  `projectsync::run_scheduled`) fetches due mail and chat sources, but reads mail and chat items with the model only as
  a batch (`batch_ready`): once the first has waited `BATCH_WAIT` (2 h) since it was first seen waiting, or
  `BATCH_ITEMS` (5) wait, or with a call read anyway (calls keep `SETTLE`, 15 min). When each waiting item was first
  seen waiting is kept in `ledger/waiting.json` (`projectsync::waiting`, under `ledger/.waiting.lock`, re-read inside it;
  an item read since its kept time — a chat re-queued once it changed — or with a kept time that can't be read waits
  from now). A held sync ends `done` saying how many wait and when they're read — a time in the night moved to 07:00 (`projectsync::after_quiet`), and in the night itself without "or once 5 wait"; also under the other passes' reports
  when those ran. Sync now (`projectsync::run`, the window's and `ledger <id> run`)
  reads them at once.

## People

`people::decide` applies a suggestion after the quote check and the user's yes; `people::refresh_m365` turns a change to
a field or role the user set into a question.

## Microsoft 365 sends

- A command that sends, replies, posts or uploads returns `{sent: false, would}` without `--yes` (`m365::mail_send`).
- `m365::not_in_run` runs before a send: refused when `ECHOCTL_HEADLESS` is set, but for a routine's or Yolo's run acting
  on its own (`.ai-run/guides/security/untrusted-text.md` R21).
