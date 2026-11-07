---
id: TC-005
title: A session can't take a call's minutes while Echo writes them; Write minutes takes them from a session
priority: medium
type: functional
requirements: [F-CC-03, F-MIN-11]
module: claude
run: agent
platform: macos
tags: [minutes, claim, test-call]
ai: local
external_id: TC-CLA-004, TC-CLA-013
---

# TC-005: A session can't take a call's minutes while Echo writes them; Write minutes takes them from a session

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model linked into the test config (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, `target/debug/echoctl model use large-v3-turbo-q5_0`, `config --languages en`); TestCall, BlackHole 2ch and ffmpeg as in `recording/TC-014`
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- A transcribed call whose minutes Echo is writing (stage `claude/TC-004` step 4 in S-19, or click "Write minutes" on any call without minutes)
- Initech planning (test data): transcribed, no minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | While the window shows "Writing minutes…" for a call: `. <dir>/env; target/debug/echoctl claim <that call's folder> --for "$ECHOCTL_HOME" --session test-session-1` (what `/echo:process-call` does first in a session) | Exit code 1, "Error: Echo is writing the minutes for this call right now"; nothing written by it |
| 2 | `target/debug/echoctl claim <Initech planning's folder> --for "$ECHOCTL_HOME" --session test-session-2` | Exit code 0; the folder has `.claimed` |
| 3 | In the window open Initech planning → Minutes: "No minutes yet", "Write minutes"; click it | The run starts (or waits its turn behind another run on the same model: "Minutes are waiting their turn"); the server's output says "a Claude Code session's claim taken over by Write minutes"; the minutes are written |

## Expected Final State
Echo's run keeps a session out; the user's explicit Write minutes takes a call over from a session (an automatic run would not: `notes::claim`).

## Notes
- Earlier this case expected step 3 to refuse ("Couldn't write minutes: a session is writing them"); `notes::claim` lets an explicit Write minutes take over a session's claim and refuses only automatic runs, and says so in its log.
- `claim` stands in for the session: a session's own words ("Echo is writing the minutes…") and a session writing the minutes again with follow-ups need a signed-in Claude Code session (person).
- Run on 2026-11-07 (S-19): PASS on the steps above.
