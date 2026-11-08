---
id: TC-018
title: A call's minutes follow the template whose words are in its title, else Default, and a chosen one wins (a real run on the local model)
priority: high
type: functional
requirements: [F-TPL-06]
module: templates
run: agent
platform: macos
data: standard
ai: local
tags: [templates, minutes, local-model, claude-code]
---

# TC-018: A call's minutes follow the template whose words are in its title, else Default, and a chosen one wins (a real run on the local model)

**Module:** Templates | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status` (for example "Qwen · this computer" and `qwen3.8:27b-mlx`); the fake `claude` passes only runs routed to that profile, so no Claude account is used
- **One model run** (step 3), about 2 minutes on the network test server, 10 on a Mac running the model itself. Model text differs from run to run: assert only files, states, template names and section headings, never wording. Don't retry a failed run unless it failed for an infrastructure reason (Ollama down, model missing): then the case is BLOCKED
- Find a folder by its slug: `ls $ECHOCTL_HOME/calls | grep <slug>`; `<standup>` below is the folder ending in `-globex-standup`
- **In S-30** `<profile>`, `<model>` and the call folders are read once, in the scenario's prelude (`echoctl ai status` and one `ls` of the calls), and every stage uses them. On its own, read them as above; in S-30 the Calls list is also already on screen, so step 2 starts at its click

## Test Data

| Call | Title words | Template (`echoctl context` → `template.name`) |
|---|---|---|
| Globex standup (`globex-standup`, transcript, no minutes, in Globex pilot) | "standup" (Status sync's words: daily, sync, standup, stand-up, status, weekly) | `status-sync`: Summary, Progress, Blockers & risks, Action items |
| Initrode demo (`initrode-demo`) | none of any template's | `default`: Summary, Decisions, Action items |
| Analyst briefing (`analyst-briefing`) | none, but `meta.json` → `template: "discussion"` (chosen for the call) | `discussion` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl context <folder>` for each call of the table and read `template.name` and the `template.sections[].heading` | Globex standup `status-sync` with Summary, Progress, Blockers & risks, Action items; Initrode demo `default` with Summary, Decisions, Action items; Analyst briefing `discussion` |
| 2 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="acme-renewal-negotiation"]` (minutes, no template words in its title), open "Call actions" | Under "Write again with" the items are Discussion, Status sync, Client kickoff — not Default (the one its minutes follow) and not Memo; then Escape |
| 3 | Note the time; run `target/debug/echoctl notes <standup> --wait` (Bash timeout 20 minutes) | It ends with exit 0 and the line `Minutes written: …/<standup>/notes.md`; note how long it took |
| 4 | Run `cat $ECHOCTL_HOME/calls/<standup>/notes-run.json` | `state` "done", `profile` "ollama", `model` "<profile> · <model>"; `minutes.template` "status-sync", `minutes.model` the same model |
| 5 | Run `grep '^## ' $ECHOCTL_HOME/calls/<standup>/notes.md` | Exactly `## Summary`, `## Progress`, `## Blockers & risks`, `## Action items`, in that order (no "Decisions", no "Options considered"); the call's `heading_budget` is 4, so none may be dropped |
| 6 | Run `ls $ECHOCTL_HOME/calls/<standup>/` and `target/debug/echoctl actions <standup>` | `notes.md`, `actions.json`, `claude.log` and `Globex standup-minutes.html` (the minutes page is on in this data) are there; `actions` prints a JSON list of at least one item, each with `id`, `owner`, `task`, `status` "todo" |
| 7 | Reload `{{base_url}}/`, click the row `[data-call$="globex-standup"]`, tab "Minutes" | The minutes frame's first line reads "Written by AI (Echo, <profile> · <model> · Status sync template) from an automatic transcript…", then the headings Summary, Progress, Blockers & risks, Action items; no "Couldn't write the minutes" box or failure bar |
| 8 | Open "Call actions" | "Write again with" lists Default, Discussion, Client kickoff — not Status sync; Escape closes the menu |

## Expected Final State
Globex standup has minutes in the Status sync template, written by the local model, with its action items and its minutes page; no other call changed.

## Notes
- Verified 2026-11-07: 1 min 38 s on the network test server (the same four headings); 9 min 47 s (`notes --wait`, 587 s) on this Mac's Ollama; `minutes.template` "status-sync", the four headings exactly, 7 action items, the minutes page written.
- The window gives no other place that names the template of a call without minutes; `echoctl context` is what the minutes run itself reads.
