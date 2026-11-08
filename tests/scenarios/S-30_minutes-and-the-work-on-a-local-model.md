---
id: S-30
title: "Minutes and the work on a local model: no Claude Code, a template by title, write again with another, a memo, a thread's sync, Yolo, a prep, and a call's profile"
run: agent
data: standard
ai: local
cases:
  - tests/cases/calls/TC-037_without-claude-code-or-its-plugin-the-minutes-tab-says-what-to-do.md
  - tests/cases/templates/TC-018_minutes-follow-the-template-whose-words-are-in-the-title.md
  - tests/cases/claude/TC-016_write-again-with-another-template-rewrites-the-minutes-and-keeps-it.md
  - tests/cases/claude/TC-017_a-memo-gets-minutes-in-the-memo-template-by-thread.md
  - tests/cases/prep/TC-015_prep-write-on-the-local-model-writes-a-summary.md
  - tests/cases/tasks/TC-026_yolo-on-the-local-model-does-the-task-and-says-so-above-it.md
  - tests/cases/work/TC-028_sync-on-the-local-model-reads-a-call-and-keeps-what-you-set.md
  - tests/cases/calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md
---

# S-30: Minutes and the work on a local model

**Story:** Sam has no Claude account on this computer: his AI profile is a model server of his own (Ollama, on this Mac or one on the network). First he sees what Echo says on a computer without Claude Code or without Echo's plugin. Then Claude Code writes real minutes on the local model: a standup's follow the Status sync template because of its title, a negotiation's are written again with Discussion and keep it, a memo's come in the memo template, one part per thread. He has a meeting's prep written, gives a task to Claude with Yolo, syncs Acme rollout's work after editing two of its items himself, and last checks that a call keeps the profile it was given when he changes the default.

**Data:** `standard` with the local model (`tests/fixtures/serve.sh up <dir> <port> --set standard --ai local`; `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). Fresh data per run, built right before it, between 06:00 and 20:00 local (the prep stage needs a meeting still to come). Needs Claude Code on `PATH` and the data's model server answering. Serve it with `serve.sh` only, never the Echo app. Ports: the scenario's, and its port + 1 and + 2 for stage 1.

**Length:** about 20 minutes of model time on the network test server (stage 2 ≈ 2 min, 3 ≈ 1 min, 4 ≈ 1 min, 5 seconds, 6 ≈ 4–7 min, 7 ≈ 5 min); about 1 h 45 min on a Mac running the model itself (7 ≈ 45 min, 6 may hit Yolo's 20-minute limit). One model job per stage from 2 to 7 (six in all; the sync is three passes), each on its own prompt or condition, none repeated; a stage whose run fails for an infrastructure reason (Ollama down, model missing) is BLOCKED, not retried.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`standard --ai local`) | the lead (`serve.sh up … --set standard --ai local`), right before the run, between 06:00 and 20:00 | all |
| `<profile>`, `<model>` and `<where>` (the default AI profile in `echoctl ai status`) | Prelude, step P2 | 2–8 |
| The call folders (`<standup>`, Initrode demo, Analyst briefing, `<call>` = Acme renewal negotiation, the memo's `<stamp>`, Umbrella proposal walkthrough) | Prelude, step P2 | 2–4, 8 |
| Two extra window servers (no Claude Code; no plugin) | stage 1 (steps 1, 3) | 1 (stopped in its teardown) |
| Model run 1: Globex standup's minutes, template by title (`notes --wait`) | stage 2 (step 3) | 2 (notes-run, headings, actions, minutes page, frame line, menu) |
| Model run 2: Acme renewal negotiation written again with Discussion | stage 3 (step 2) | 3 (menus, meta, context, notes-run, headings, frame line) |
| Model run 3: the memo's minutes by thread | stage 4 (step 2) | 4 (notes-run, renamed folder, headings, parts, frame); 7 reads the memo again as pending evidence |
| Model job 4: Acme renewal follow-up's prep (one request, no Claude Code) | stage 5 (step 2) | 5 |
| Model run 5: Yolo on T005 | stage 6 (step 3) | 6 (bar, task-asks, log, drafts, file diff, git, History) |
| Model run 6: Acme rollout's sync (extract, verify, people) | stage 7 (step 3) | 7 (status, pending, run folder, tree, pinned fields, log, window) |

## Prelude

Done once, before stage 1, timed as the stage `prelude`.

| # | Action | Expected Result |
|---|---|---|
| P1 | Load `{{base_url}}/` (the cookie) in the open browser | The window loads |
| P2 | `. <dir>/env; target/debug/echoctl ai status; ls "$ECHOCTL_HOME/calls" \| grep -E -- '-(globex-standup\|initrode-demo\|analyst-briefing\|acme-renewal-negotiation\|memo\|umbrella-proposal-walkthrough)$'` | The default profile is the local one (`ollama`): note its `name` (`<profile>`), its `notes_model` (`<model>`) and `<where>` ("on this computer" for a model on this Mac, "at <host>" for one on the network); note the folders. Every later stage uses these instead of reading `ai status` or looking a folder up again (the memo's folder is renamed by stage 4: find it by `<stamp>` after that) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-037_without-claude-code-or-its-plugin-the-minutes-tab-says-what-to-do.md` | — | — (no model run) |
| 2 | `templates/TC-018_minutes-follow-the-template-whose-words-are-in-the-title.md` | — | `<profile>`, `<model>` and the folders from the prelude. On stage 1's view (`{{base_url}}`, the Calls list): in step 2 skip the navigation, click the row. Step 1's three `context` reads in one shell call |
| 3 | `claude/TC-016_write-again-with-another-template-rewrites-the-minutes-and-keeps-it.md` | — | `<profile>`, `<model>`, `<call>` from the prelude. On stage 2's view (the Calls list): in step 1 skip the navigation, click the row and the tab |
| 4 | `claude/TC-017_a-memo-gets-minutes-in-the-memo-template-by-thread.md` | — | `<profile>`, `<model>`, `<stamp>` from the prelude. On stage 3's view (the Calls list): in step 1 skip the navigation, click the row. Steps 4–6 in one shell call (each command preceded by `echo "== step N"`) |
| 5 | `prep/TC-015_prep-write-on-the-local-model-writes-a-summary.md` | — | `<model>` from the prelude. Steps 3–4 in one shell call |
| 6 | `tasks/TC-026_yolo-on-the-local-model-does-the-task-and-says-so-above-it.md` | — | `<profile>`, `<model>` from the prelude. Step 1's commands in one shell call |
| 7 | `work/TC-028_sync-on-the-local-model-reads-a-call-and-keeps-what-you-set.md` | — | `<profile>`, `<model>` from the prelude. Stage 4 renamed the memo: its evidence id in step 2 comes from `$L pending` as the case says |
| 8 | `calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md` | — | `<profile>`, `<model>`, `<where>` from the prelude. Reload `{{base_url}}/` |

## Notes
- Six model jobs, each a different prompt or condition: a template chosen by the title (CLI), a template chosen in the window on a call with minutes, the memo template by thread, a prep summary (one request), Yolo on a task, a thread's sync. No check needs a second run: each run's minutes, files, log and window are read by the stage that started it, and stage 7 reads the memo's run (stage 4) only as evidence already done.
- Model output differs from run to run: the stages assert files, states, template names, headings and the window around the output, never its wording.
- Stage 4 renames the memo's folder (the run titles it); stage 7 changes Acme rollout's work; stage 8 changes the default AI profile and puts it back. Stage 2 must run before stage 3 reads the "Write again with" menus. Don't run stages side by side: stages 6 and 7 diff the whole data folder and the ledger around their own run.
- Nothing here signs in to anything: the fake `claude` refuses every run not routed to the local model.
