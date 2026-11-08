---
id: TC-005
title: A session can't take a call's minutes while Echo writes them; Write minutes takes them from a session
priority: medium
type: functional
requirements: [F-CC-03, F-MIN-11]
module: claude
run: agent
platform: macos
tags: [minutes, claim]
ai: local
external_id: TC-CLA-004, TC-CLA-013
---

# TC-005: A session can't take a call's minutes while Echo writes them; Write minutes takes them from a session

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D` (the env sets the file-fed call sources and the call app's id: nothing here records)
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- **A call whose minutes Echo is writing** (step 1). In S-19: stage 1's long call during its step 4 (run A, in folder A). On its own: click "Write minutes" on any transcribed call without minutes
- **A transcribed call without minutes that nobody is writing** (steps 2–3). In S-19: stage 1's short call "CLA-004 short" after its step 6, and step 3's click is that stage's step 7 (run B: one run for both cases). On its own: Initech planning (test data)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | While the window shows "Writing minutes…" for a call: `. <dir>/env; target/debug/echoctl claim <that call's folder> --for <the session's folder> --session test-session-1` (what `/echo:process-call` does first in a session; the session's folder is `$ECHOCTL_HOME`, or the folder `config --notes-in` set, e.g. `"$D/folder-a"` in S-19: a session outside it is refused for that first) | Exit code 1, "Error: Echo is writing the minutes for this call right now"; nothing written by it |
| 2 | `target/debug/echoctl claim <the call without minutes> --for "$ECHOCTL_HOME" --session test-session-2` (with `notes_in` unset) | Exit code 0; the folder has `.claimed` |
| 3 | In the window open that call → Minutes: "Write minutes" (under "No minutes yet" for an old call like Initech planning; under "A Claude Code session is writing the minutes", with the session's process, for a call due for automatic ones); click it | The run starts (or waits its turn behind another run on the same model: "Minutes are waiting their turn"); the server's output says "a Claude Code session's claim taken over by Write minutes"; the minutes are written |

## Expected Final State
Echo's run keeps a session out; the user's explicit Write minutes takes a call over from a session (an automatic run would not: `notes::claim`).

## Notes
- Earlier this case expected step 3 to refuse ("Couldn't write minutes: a session is writing them"); `notes::claim` lets an explicit Write minutes take over a session's claim and refuses only automatic runs, and says so in its log.
- `claim` stands in for the session: a session's own words ("Echo is writing the minutes…") and a session writing the minutes again with follow-ups need a signed-in Claude Code session (person).
- Since 2026-11-08 S-19 runs no model for this case: step 1 is during `claude/TC-004`'s run A, steps 2–3 are on its short call and run B. A claim with `--for "$ECHOCTL_HOME"` while `notes_in` is folder A is refused with "this session isn't in the folder set by `echoctl config --notes-in`" before the call is looked at (`calls::claim`): step 1 uses the session folder Echo allows.
- Run on 2026-11-07 (S-19): PASS on the steps above.
