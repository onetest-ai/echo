---
id: TC-032
title: With a template that doesn't keep every section, a heading in the user's notes becomes a section of its own before the template's (a real run on the local model)
priority: medium
type: functional
requirements: [F-MIN-20]
module: claude
run: agent
platform: macos
data: standard
ai: local
tags: [claude, minutes, templates, local-model]
---

# TC-032: With a template that doesn't keep every section, a heading in the user's notes becomes a section of its own before the template's

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model")
- **One model run** (started in step 3), about 1–2 minutes on the network test server, 8–10 on a Mac running the model itself. Assert files, states and headings, never the model's wording; no retry unless the run failed for an infrastructure reason (then BLOCKED)
- `<call>` below is the folder ending in `-globex-pilot-kickoff` (`ls $ECHOCTL_HOME/calls | grep globex-pilot-kickoff`): its title's "kickoff" picks the user's template Client kickoff, which doesn't keep every section
- **In S-30** `<call>` is read once, in the scenario's prelude (step P2)

**Changes:** the call's `my-notes.md`, `notes.md`, `notes-run.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `<call>` in Calls, its My notes, "Edit as text" (`textarea#my-notes`); at the end add the lines `# Logistics` and `- The pilot room is booked for the first week`; wait for "Saved" | The notes are saved |
| 2 | Run `target/debug/echoctl context <call>` | `template.name` `client-kickoff`, `template.fixed` `false`, `heading_budget` a number, `user_headings` contains `Logistics` |
| 3 | In the call's Minutes tab press "Write minutes" (or, if the call has minutes, "Write minutes again") | Within 15 seconds the tab shows "Writing minutes…" or "Updating the minutes…" |
| 4 | Wait until `cat $ECHOCTL_HOME/calls/<call>/notes-run.json` shows a `state` other than "queued"/"running" (every 30 seconds, up to 20 minutes) | `state` "done" |
| 5 | Run `grep -n '^##' $ECHOCTL_HOME/calls/<call>/notes.md` | A `## Logistics` heading comes before every heading of the Client kickoff template; no `### Logistics` |
| 6 | Read the lines under `## Logistics` | They hold the point about the pilot room |

## Expected Final State
The call's minutes have the user's heading as a section of its own, before the template's sections, as for any template that doesn't keep every section.

## Teardown
- None (S-30's data folder is thrown away after the run).
