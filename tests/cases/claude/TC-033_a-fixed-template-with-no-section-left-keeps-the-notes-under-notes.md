---
id: TC-033
title: A template that keeps every section but has none that applies puts the user's heading under one Notes section (a real run on the local model)
priority: medium
type: functional
requirements: [F-MIN-23]
module: claude
run: agent
platform: macos
data: standard
ai: local
tags: [claude, minutes, templates, local-model]
---

# TC-033: A template that keeps every section but has none that applies puts the user's heading under one Notes section

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model")
- **One model run** (started in step 4), about 1–2 minutes on the network test server, 8–10 on a Mac running the model itself. Assert files, states and headings, never the model's wording; no retry unless the run failed for an infrastructure reason (then BLOCKED)
- `<call>` below is the folder ending in `-acme-renewal-negotiation` (`ls $ECHOCTL_HOME/calls | grep acme-renewal-negotiation`): no chat and no shared screens, so no stream section is added; its made-up transcript has no office move
- **In S-30** `<call>` is read once, in the scenario's prelude (step P2)

**Changes:** `<data>/.echo/templates/only-if.md`, the call's `my-notes.md`, `notes.md`, `notes-run.json`, `meta.json` (`template`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/templates`, click "New template", name it `Only if`; replace its sections with one section `Office move` whose words are `Only when an office move was decided: what`; tick "Keep every section"; click "Create" | A row "Only if" with the tag "Yours" reads "1 section, all kept" |
| 2 | Open `<call>` in Calls, its My notes, "Edit as text" (`textarea#my-notes`); at the end add the lines `# Fallback topic` and `- Check the cap with finance`; wait for "Saved" | The notes are saved |
| 3 | Run `target/debug/echoctl context <call>` | `user_headings` contains `Fallback topic` |
| 4 | In the call's Minutes tab open "Call actions", under "Write again with" click "Only if" | Within 15 seconds the tab shows "Updating the minutes…" |
| 5 | Wait until `cat $ECHOCTL_HOME/calls/<call>/notes-run.json` shows a `state` other than "queued"/"running" (every 30 seconds, up to 20 minutes) | `state` "done" |
| 6 | Run `grep -n '^## ' $ECHOCTL_HOME/calls/<call>/notes.md` and `grep -n '^### ' $ECHOCTL_HOME/calls/<call>/notes.md` | The first prints exactly one line, `## Notes`; the second includes `### Fallback topic`, after it; no `## Office move` |
| 7 | Read the lines under `### Fallback topic` | They hold the point about the cap and finance |

## Expected Final State
The call's minutes hold only a Notes section with the user's topic; the template's one section, which didn't apply, is left out.

## Teardown
- In `{{base_url}}/#settings/templates` open "Only if" and click "Delete".
