---
id: TC-017
title: A memo's minutes, written on the local model, follow the memo template with one part per thread, a title of their own and the minutes page beside them
priority: low
type: functional
requirements: [F-MIN-17]
module: claude
run: agent
platform: macos
data: standard
ai: local
tags: [claude, minutes, memo, local-model]
---

# TC-017: A memo's minutes, written on the local model, follow the memo template with one part per thread, a title of their own and the minutes page beside them

**Module:** Claude Code | **Priority:** Low | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Claude Code on `PATH` and the data's model server answering (`--ai local`: `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status` (for example "Qwen · this computer" and `qwen3.8:27b-mlx`); the fake `claude` passes only runs routed to that profile, so no Claude account is used
- **One model run** (step 2), about 4 minutes. Assert files, states, headings and the window, never the model's wording; no retry unless it failed for an infrastructure reason (then BLOCKED)
- Today's memo is the folder `<stamp>-memo` (`ls $ECHOCTL_HOME/calls | grep -- '-memo$' | tail -1`; `<stamp>` is its `YYYY-MM-DD_HHMMSS` part). Its transcript is one line about Acme rollout and Globex pilot; it is in both threads (`memo-threads.json`) and has made-up minutes. **The run gives the memo a title and renames its folder**: after it, find the call by `<stamp>` (`ls -d $ECHOCTL_HOME/calls/<stamp>-*`)
- The data's settings have the minutes page on (`config --minutes-page true`)

## Test Data

| Memo | Before | After |
|---|---|---|
| Title / folder | "Memo" / `<stamp>-memo` | a title of the run's (not "Memo") / `<stamp>-<its slug>` |
| `notes.md` headings | `# Memo`, `## Summary` | `## Summary`, `## Per thread` with one `### Acme rollout (…)` and one `### Globex pilot (…)`, in that order |
| Menu | no "Write again with" (a memo has only the memo template) | the same |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="-memo"]` whose folder starts with `<stamp>`; open "Call actions" | The header reads "Memo", the kind "Memo", chips "Acme rollout 0:00–0:06" and "Globex pilot 0:06–0:12"; the menu holds Show folder, Open minutes in browser, Show minutes in Finder, Write minutes again, Ask Claude about this call, Delete… — no "Write again with" label and no template items |
| 2 | Note the time; click "Write minutes again" | Within 15 seconds the Minutes tab shows "Updating the minutes…" with "Show log" |
| 3 | Every 30 seconds run `cat $ECHOCTL_HOME/calls/<stamp>-*/notes-run.json` until `state` is no longer "queued"/"running" (up to 15 minutes); note how long it took | `state` "done", `profile` "ollama", `had_minutes` true; `minutes.model` "<profile> · <model>" and **no** `minutes.template` (a memo's template is not named) |
| 4 | Run `ls $ECHOCTL_HOME/calls | grep <stamp>` and `grep '"title"' $ECHOCTL_HOME/calls/<stamp>-*/meta.json` | One folder, `<stamp>-<slug>` with a slug other than `memo`; `meta.json` `title` is the new title (not "Memo") and `kind` stays "memo" |
| 5 | Run `grep '^#' $ECHOCTL_HOME/calls/<stamp>-*/notes.md` | `# <the new title>`, `## Summary`, `## Per thread`, `### Acme rollout (00:00–00:06)` and `### Globex pilot (00:06–00:12)` (each thread name may be a link `[Acme rollout](echo://thread/acme-rollout)`) — the two parts in the memo's order, each with its span (a part about no thread would be `### Other`) |
| 6 | Run `cat $ECHOCTL_HOME/calls/<stamp>-*/memo-threads.json` and `ls $ECHOCTL_HOME/calls/<stamp>-*/` | `parts` acme-rollout 0–6 and globex-pilot 6–12, `at` from this run; the folder holds `notes.md`, `actions.json`, `claude.log` and `<new title>-minutes.html` (the minutes page, written beside the notes) |
| 7 | Reload `{{base_url}}/`, open `#calls`, click the row whose `data-call` starts with `<stamp>` | The header shows the new title above "Memo"; the thread chips are unchanged; the minutes frame's first line reads "Written by AI (Echo, <profile> · <model>) from an automatic transcript…", then Summary, Per thread and the two thread headings, each a link to its thread |
| 8 | Open "Call actions" | Still no "Write again with"; Escape closes it |

## Expected Final State
The memo has minutes in the memo template, written by the local model, one part per thread; it is titled and its folder renamed; the minutes page is beside the notes.

## Notes
- Verified 2026-11-07: 3 min 36 s (216 s) on this Mac's Ollama, 54 s on the network test server (there the title was "Omar reminder and Globex pilot dates" and the thread names plain text). Title "Acme rollout reminder and Globex pilot dates"; headings and parts exactly as in step 5. The log held two `✗` lines, both harmless: the guard refused a `Glob` with no path ("Echo's minutes run: Glob needs a path in this call's folder"), and an `echoctl title` without a title failed on its arguments.
- The page open on the old folder logs two 404s (`/api/call/<stamp>-memo` and `…/rev`) once the folder is renamed, then follows the new name by itself: not a failure.
- "At once" — the app writes a memo's minutes without the 15-minute wait — is the app's scheduler (`echoctl app`), not `window --serve`: not covered here.
