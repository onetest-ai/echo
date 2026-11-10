---
id: S-30
title: "Minutes on a local model: no Claude Code, a template by title, write again with another, a memo, and a call's profile"
run: agent
data: standard
ai: local
cases:
  - tests/cases/calls/TC-037_without-claude-code-or-its-plugin-the-minutes-tab-says-what-to-do.md
  - tests/cases/templates/TC-018_minutes-follow-the-template-whose-words-are-in-the-title.md
  - tests/cases/claude/TC-016_write-again-with-another-template-rewrites-the-minutes-and-keeps-it.md
  - tests/cases/claude/TC-017_a-memo-gets-minutes-in-the-memo-template-by-thread.md
  - tests/cases/calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md
---

# S-30: Minutes on a local model

**Story:** Sam has no Claude account on this computer: his AI profile is a model server of his own (Ollama, on this Mac or one on the network). First he sees what Echo says on a computer without Claude Code or without Echo's plugin. Then Claude Code writes real minutes on the local model: a standup's follow the Status sync template because of its title, a negotiation's are written again with Discussion and keep it, a memo's come in the memo template, one part per thread. Last he checks that a call keeps the profile it was given when he changes the default.

**Data:** `standard` with the local model (`tests/fixtures/serve.sh up <dir> <port> --set standard --ai local`; `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). Fresh data per run, built right before it, between 06:00 and 20:00 local. Needs Claude Code on `PATH` and the data's model server answering. Serve it with `serve.sh` only, never the Echo app. Ports: the scenario's, and its port + 1 and + 2 for stage 1.

**Length:** about 4 minutes of model time on the network test server (stage 2 ≈ 2 min, 3 ≈ 1 min, 4 ≈ 1 min); longer on a Mac running the model itself. One model run per stage from 2 to 4 (three in all), each on its own prompt or condition, none repeated; a stage whose run fails for an infrastructure reason (Ollama down, model missing) is BLOCKED, not retried.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Data build (`standard --ai local`) | the lead (`serve.sh up … --set standard --ai local`), right before the run, between 06:00 and 20:00 | all |
| `<profile>`, `<model>` and `<where>` (the default AI profile in `echoctl ai status`) | Prelude, step P2 | 2–5 |
| The call folders (`<standup>`, Initrode demo, Analyst briefing, `<call>` = Acme renewal negotiation, the memo's `<stamp>`, Umbrella proposal walkthrough) | Prelude, step P2 | 2–5 |
| Two extra window servers (no Claude Code; no plugin) | stage 1 (steps 1, 3) | 1 (stopped in its teardown) |
| Model run 1: Globex standup's minutes, template by title (`notes --wait`) | stage 2 (step 3) | 2 (notes-run, headings, actions, minutes page, frame line, menu) |
| Model run 2: Acme renewal negotiation written again with Discussion | stage 3 (step 2) | 3 (menus, meta, context, notes-run, headings, frame line) |
| Model run 3: the memo's minutes by thread | stage 4 (step 2) | 4 (notes-run, renamed folder, headings, parts, frame) |

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
| 5 | `calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md` | — | `<profile>`, `<model>`, `<where>` from the prelude. Reload `{{base_url}}/` |

## Notes
- Three model runs, each a different prompt or condition: a template chosen by the title (CLI), a template chosen in the window on a call with minutes, the memo template by thread. No check needs a second run: each run's minutes, files, log and window are read by the stage that started it.
- The prep, Yolo and the thread's sync (prep/TC-015, tasks/TC-026, work/TC-028) moved to S-33, on the virtual model: what they check is what Echo gives the model and does with its answer, not the content a real model writes (#275).
- Model output differs from run to run: the stages assert files, states, template names, headings and the window around the output, never its wording.
- Stage 4 renames the memo's folder (the run titles it); stage 5 changes the default AI profile and puts it back. Stage 2 must run before stage 3 reads the "Write again with" menus.
- Nothing here signs in to anything: the fake `claude` refuses every run not routed to the local model.
