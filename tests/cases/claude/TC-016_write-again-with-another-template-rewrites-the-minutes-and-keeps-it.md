---
id: TC-016
title: Write again with another template rewrites the minutes in it on the local model, keeps it for later runs, and the items are disabled meanwhile
priority: medium
type: functional
requirements: [F-MIN-15]
module: claude
run: agent
platform: macos
data: standard
ai: local
tags: [claude, minutes, templates, local-model]
---

# TC-016: Write again with another template rewrites the minutes in it on the local model, keeps it for later runs, and the items are disabled meanwhile

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status` (for example "Qwen · this computer" and `qwen3.8:27b-mlx`); the fake `claude` passes only runs routed to that profile, so no Claude account is used
- **One model run** (started in step 2), about 1–2 minutes on the network test server, 8–10 on a Mac running the model itself. Assert files, states, template names and headings, never the model's wording; no retry unless the run failed for an infrastructure reason (then BLOCKED)
- `<call>` below is the folder ending in `-acme-renewal-negotiation` (`ls $ECHOCTL_HOME/calls | grep acme-renewal-negotiation`). The minutes frame is `iframe#minutes-frame`: read it from the snapshot (its text isn't seen by `browser_wait_for`)

## Test Data

| Acme renewal negotiation | Before | After Write again with → Discussion |
|---|---|---|
| Minutes headings | Summary, Open questions (made-up minutes, no `notes-run.json`) | at most `heading_budget` (4 for this call) of Discussion's Summary, Options considered, Decisions, Action items, Open questions, in that order |
| `meta.json` → `template` | none (Default: no template words in the title) | `discussion` |
| "Write again with" items | Discussion, Status sync, Client kickoff | Default, Status sync, Client kickoff |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="acme-renewal-negotiation"]`, tab "Minutes"; open "Call actions" | Under the label "Write again with": Discussion, Status sync, Client kickoff (no Default, no Memo), then "Edit templates…"; all enabled |
| 2 | Note the time; click "Discussion" | The menu closes; within 15 seconds the Minutes tab shows the bar "Updating the minutes…" with "Show log" (the old minutes stay under it) |
| 3 | While the bar shows, open "Call actions" again | "Write again with" now lists Default, Status sync, Client kickoff, each disabled with the tooltip "The minutes are being written now"; Escape closes the menu |
| 4 | Run `grep '"template"' $ECHOCTL_HOME/calls/<call>/meta.json` and `target/debug/echoctl context <call>` (read `template.name`, `heading_budget`) | `"template": "discussion",`; `template.name` "discussion"; `heading_budget` 4 |
| 5 | Wait until `cat $ECHOCTL_HOME/calls/<call>/notes-run.json` shows a `state` other than "queued"/"running" (check every 30 seconds, up to 20 minutes); note how long it took | `state` "done", `profile` "ollama", `had_minutes` true, `minutes.template` "discussion", `minutes.model` "<profile> · <model>" |
| 6 | Run `grep '^## ' $ECHOCTL_HOME/calls/<call>/notes.md` | 3 or 4 headings, every one of them a Discussion section and in Discussion's order; at least one of "Options considered" and "Open questions" (Default has neither) |
| 7 | Reload `{{base_url}}/`, open the call's Minutes tab, snapshot the frame | No bar; the frame's first line reads "Written by AI (Echo, <profile> · <model> · Discussion template) from an automatic transcript…"; its headings are those of step 6 |
| 8 | Open "Call actions" | "Write again with" lists Default, Status sync, Client kickoff, enabled — not Discussion: the call keeps Discussion for every later run (Write minutes again uses it too) |

## Expected Final State
Acme renewal negotiation's minutes are written again by the local model in the Discussion template, which is now the call's own (`meta.json`); nothing is running.

## Teardown
- None (the call keeps Discussion; later stages that read its minutes expect the new ones)

## Notes
- Verified 2026-11-07: 1 min 9 s on the network test server (same headings); 8 min 18 s (498 s) on this Mac's Ollama; headings Summary, Decisions, Action items, Open questions (Options considered dropped: the skill keeps to `heading_budget`); the frame line and both menus exactly as above.
- The run also set the call's kind to Steering (`echoctl type` is one of the run's commands): don't assert the kind chip after it.
- "Later runs keep it" is checked through the menu and `context` (what the next run reads), not by a second run.
