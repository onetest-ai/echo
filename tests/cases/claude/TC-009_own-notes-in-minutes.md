---
id: TC-009
title: The user's notes, from during and after the call, shape its minutes
priority: high
type: functional
requirements: [F-MIN-20, F-MIN-13]
module: claude
run: agent
platform: macos
tags: [notes, minutes, recordings]
ai: local
external_id: TC-CLA-008
---

# TC-009: The user's notes, from during and after the call, shape its minutes

**Module:** Claude | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai local --clock fast` (`ai: local`): the small set, hermetic, with the AI profile "Qwen" (a local model server: Ollama here, or the faster one of `tests/fixtures/ai.local.json`) the default, so a minutes run is a real Claude Code run on it and every other run is refused (`tests/fixtures/README.md`, "Real Claude Code runs on a local model"); Claude Code on `PATH`
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`); every command after `. <dir>/env`
- The window served on the data with clicks reaching it (`ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 … window --serve <port>`, or the scenario's server), the page loaded from `{{base_url}}/` first
- **The call and its run (shared in S-19):** a transcribed `minutes-call` copy with the three notes below in its `my-notes.md`, its stamp (the folder name's first 17 characters, kept when a run renames the folder) in `$(dirname "$ECHOCTL_HOME")/min-a.stamp`, and its minutes written after the notes. In S-19: stage 1's long call ("CLA-004 write now"): the scenario puts the notes in right after `claude/TC-004`'s step 1, and its step 4 (run A) writes the minutes, so steps 1 and 3 are skipped. On its own: step 1 makes the call and step 3 writes them

## Test Data

| Field | Value |
|-------|-------|
| Call | a copy of `minutes-call` (213 s), title "MIN-A catch-up" (in S-19 "CLA-004 write now") |
| Notes during the call (as `echoctl note` and the window write them while recording: the call's time in brackets) | `- [00:09] ask legal about the contract`, `- [00:14] Berlin needs ten seats in the training room` |
| Note after the call (no time) | `- Ask Kim for the pilot budget` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `CALL="$(python3 tests/fixtures/recordings.py copy minutes-call --title "MIN-A catch-up")"; printf -- '- [00:09] ask legal about the contract\n- [00:14] Berlin needs ten seats in the training room\n- Ask Kim for the pilot budget\n' > "$CALL/my-notes.md"; basename "$CALL" \| cut -c1-17 > "$(dirname "$ECHOCTL_HOME")/min-a.stamp"; echo "$CALL"` | The call's folder, with its `my-notes.md`; its stamp saved |
| 2 | In the window: `{{base_url}}/#calls`, click the row `[role=option][data-call^="<the stamp>"]`, open My notes | My notes lists the three notes, the first two with their times `00:09` and `00:14`, the third without |
| 3 | Minutes tab → "Write now" (`[data-cmd="write-notes"]`; with `--clock fast` within 90 s of step 1: after that, with no app running, the tab says "Starting the minutes…" without it) | The run starts ("Writing minutes…") |
| 4 | Wait until the call's `notes-run.json` says `"state": "done"` (`S=$(cat "$(dirname "$ECHOCTL_HOME")/min-a.stamp"); python3 tests/fixtures/wait.py 600 sh "grep -qs '\"state\": *\"done\"' $ECHOCTL_HOME/calls/$S*/notes-run.json"`; the run may retitle and rename the folder) | The minutes are written (`notes.md`, `actions.json`) |
| 5 | `grep -n -i -e "legal" -e "ten seats" -e "pilot budget" "$ECHOCTL_HOME"/calls/$(cat "$(dirname "$ECHOCTL_HOME")/min-a.stamp")*/notes.md` | All three notes are in the minutes, in their structure (e.g. an action item "Me — Ask legal about the contract", "ten seats" with the training room, "Me — ask Kim for the pilot budget"), not pasted as a list of notes |

## Expected Final State
The user's own notes, taken during the call and after it, are part of the minutes.

## Notes
- That notes typed while recording carry the call's time is `window/TC-014` (a live recording); `echoctl note` writes what the tray's "Add a note…" writes (`calls::note`). Here the notes file is made-up data in that same form, so the case judges only what the model does with it.
- The model's wording differs from run to run: the check is that each note's content is there.
- Moved from S-17 to S-19 on 2026-11-08 (S-17 now runs on the fake: it checks the timing, not what a model writes). Before, it recorded a 213 s call through the watcher and waited 15 minutes for the automatic run.
- Since 2026-11-08 S-19 runs it on `claude/TC-004`'s long call and run A (one run for both cases): the notes go in before that run, and the call is found by its stamp, not its title. The step order changed with it (the click and the wait are steps 3 and 4).
- Run on 2026-11-07 (S-17, the network model server): PASS — "Me — Ask legal about the contract (user note, [00:09])" in Action items, "Berlin needs ten seats in it (user note)" in the Highlights; the run took 116 s.
