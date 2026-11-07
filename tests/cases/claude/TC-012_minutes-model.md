---
id: TC-012
title: Each AI profile's minutes model is used, and a wrong one is explained
priority: medium
type: functional
requirements: [F-AI-01, F-SET-12, F-AI-08]
module: claude
run: agent
platform: macos
tags: [minutes, ai-profile, test-call]
ai: local
external_id: TC-CLA-011
---

# TC-012: Each AI profile's minutes model is used, and a wrong one is explained

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (Ollama here, or the faster server of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout
- Every Bash call starts with the prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model linked into the test config (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, `target/debug/echoctl model use large-v3-turbo-q5_0`, `config --languages en`); TestCall, BlackHole 2ch and ffmpeg as in `recording/TC-014`
- The window served on the data with clicks reaching it: `ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 target/debug/echoctl window --serve <port>` after the prelude (no Echo app needed: the served window starts the run itself); the page loaded from `{{base_url}}/` first
- Two AI profiles: the built-in "Claude Code (as configured)" (`system`) and "Qwen" (`ollama`, the default); a call with minutes written on Qwen (in S-19, `claude/TC-004`'s short call)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Read that call's `claude.log` and its Minutes tab | The log's `model:` line and "Claude Code started · model …" name Qwen's model; the minutes' note "Written by AI (Echo, Qwen · … · <model> …)" names the same |
| 2 | `. <dir>/env; target/debug/echoctl ai set ollama --notes-model no-such-model`; in the window, that call → "Call actions" → "Write minutes again"; wait a few seconds | Above the kept minutes: "The last update didn't finish: Claude Code can't use the model of Qwen · … (… no-such-model). Change it under menu → AI for calls → Minutes model (while it is the profile new calls use), or `echoctl ai set ollama --notes-model <name>`." with "Try again" and "Show log"; the earlier minutes stay |
| 3 | `target/debug/echoctl ai set ollama --notes-model ""` | "Qwen …: minutes model <its model>" (back to its default) |
| 4 | `target/debug/echoctl ai set system --notes-model opus`, then `target/debug/echoctl ai status` | `system` shows `notes_model` "opus"; `ollama` is unchanged |
| 5 | `target/debug/echoctl ai set system --notes-model default`, then `ai status` | `system`'s `notes_model` is null ("Claude Code's default model"): Claude Code's own setting |

## Expected Final State
The minutes model is per profile, shown where it is used, and a bad one is explained without losing the minutes.

## Notes
- A run on the built-in profile (haiku, `default`) needs a signed-in Claude account: person. The tray's "Minutes model for <profile>" sets what `ai set` sets.
- Run on 2026-11-07 (S-19): PASS.
