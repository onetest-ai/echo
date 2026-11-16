---
id: TC-006
title: Minutes run in the chosen folder
priority: low
type: functional
requirements: [F-SET-12]
module: claude
run: agent
platform: macos
tags: [minutes, settings, recordings]
ai: virtual
external_id: TC-CLA-005
auto: [tests/auto/specs/S-19.spec.mjs]
---

# TC-006: Minutes run in the chosen folder

**Module:** Claude Code | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai virtual` (`ai: virtual`): the small set, hermetic, with the AI profile "Virtual model" (`tests/fixtures/model_server.py`, answering from its default script: `echoctl context`, the transcript read, `notes.md` and the action list written and applied) the default, so a minutes run is a real Claude Code run under Echo's guard with only the model scripted (`tests/fixtures/README.md`, "The virtual model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D` (the env sets the file-fed call sources and the call app's id: nothing here records)
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- Folder A: `mkdir -p "$D/folder-a/.claude"; echo '{"env": {"ECHO_TEST_FOLDER": "A"}}' > "$D/folder-a/.claude/settings.json"`
- **Two transcribed calls without minutes, and their runs (shared in S-19).** In S-19: `claude/TC-004`'s long and short calls; its runs (steps 4 and 7: runs A and B) are this case's, with steps 1 and 3's `config` done just before them, so here only the logs are read. On its own: two copies (`recordings.py copy minutes-call`, `copy very-short`) and the Write minutes of steps 2–3

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `. <dir>/env; target/debug/echoctl config --notes-in "$D/folder-a"` | `notes_in` is folder A (the menu's "Echo runs Claude Code for minutes in" → "Choose a folder…") |
| 2 | Write minutes for the first call (window → Minutes → "Write now") and, when done, read its `claude.log` | Its header's `folder:` line names folder A |
| 3 | `target/debug/echoctl config --notes-in any`, then write minutes for the second call | `notes_in` null; that run's `folder:` line names the data folder (`$ECHOCTL_HOME`) |

## Expected Final State
Echo runs Claude Code in the folder chosen, else in the data folder; never in a user's session.

## Notes
- With the built-in profile ("Claude Code (as configured)") the folder's `.claude/settings.json` applies too (`ai::system_folder`): that run needs a signed-in Claude account, so it stays a person's check (put a gateway URL in the `env`, see it used). The local profile, run here, loads no project settings.
- Run on 2026-11-07 (S-19): PASS — folder A, then the data folder.
