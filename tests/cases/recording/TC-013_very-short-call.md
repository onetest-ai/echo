---
id: TC-013
title: A very short call gets a transcript and no automatic minutes
priority: low
type: functional
requirements: [F-MIN-08]
module: recording
run: agent
platform: macos
tags: [recording, minutes, recordings]
external_id: TC-REC-013
auto: [tests/auto/specs/S-17.spec.mjs]
---

# TC-013: A very short call gets a transcript and no automatic minutes

**Module:** Recording | **Priority:** Low | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `very-short` (Dana, then Me; 9 s) was captured from files with `echoctl capture --seconds 12` and transcribed once
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `make-data.sh`); every command after `. <dir>/env`
- The window is served on the same test data at `{{base_url}}` (`tests/fixtures/serve.sh up` or an already running scenario server), the browser open on it
- No app is needed: the Minutes tab reads the gate from the call's length. In S-17 the scenario's app is still running (it changes nothing here)

## Test Data

| Field | Value |
|-------|-------|
| Recording | `very-short`, title "REC-013 very-short" |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl config --auto-minutes true` | Exit code 0; the printed config has `auto_minutes` true (it is set before the call ends, so the call counts) |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy very-short --title "REC-013 very-short")"; ls "$CALL"; cat "$CALL/transcript.md"` | A call that ended just now: `meta.json`, `segments.json`, `transcript.md`, no `.wav`; the transcript holds "Quick one. Is the call at three still on?" (Whisper's punctuation may differ) as Remote and Me's answer |
| 3 | Navigate to `{{base_url}}/`, then to `{{base_url}}/#calls` and click the row `[role=option][data-call$="rec-013-very-short"]` ("REC-013 very-short") | The call opens; its length reads "1 min" (rounded up) |
| 4 | Open the Minutes tab | The heading "No minutes written automatically" and the line "The call is too thin for them (shorter than 3 minutes)"; a "Write minutes" button is shown (not pressed); no error text |

## Expected Final State
A 9-second call has a transcript and the Minutes tab says no minutes are written automatically because it is shorter than 3 minutes, with no error.

## Teardown
- `target/debug/echoctl config --auto-minutes false`

## Notes
- Before 2026-11-08 the case captured and transcribed its own call; the capture is now made once (`prepare`), and its words are scored in `transcription/TC-001`'s way only where that is the subject.
- Run 2026-11-08 evening (S-17, CLI part; the Minutes tab read through `/api/call/<name>`: `auto` `{"due": "gate", "reason": "shorter than 3 minutes"}`): PASS on steps 1–2.
- Run on 2026-11-07 (S-17, TestCall), with `--seconds 16`: a line said in the room after the call (#140) failed the transcript; file-fed recordings can't pick up the room (the live check's: `recording/TC-023`).
- Per `calls::gate`, every call under 3 minutes is gated, so the acme-sync test call (36 s) would show the same text.
