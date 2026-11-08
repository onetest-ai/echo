---
id: TC-017
title: Twenty seconds of silence in a recorded call give no restart and no text
priority: medium
type: functional
requirements: [F-STT-03, F-REC-07]
module: recording
run: agent
platform: macos
tags: [recording, file-fed, silence, recordings]
---

# TC-017: Twenty seconds of silence in a recorded call give no restart and no text

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `long-silence` (Me, 20 s of nobody speaking, Dana, Me; 36 s) was fed from files to `echoctl capture --seconds 39` (no device: app_profile.md → "Recording with file-fed calls") and transcribed once. This case reads that capture; it records and transcribes nothing
- Every command after `. <dir>/env` on test data built by `tests/fixtures/make-data.sh <dir>`, and `P="$(python3 tests/fixtures/recordings.py path)/long-silence"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -E "restarting|(mic|remote)\.wav" "$P/capture.err"` | Both tracks' lines (`mic.wav`, `remote.wav`) with `0 restart(s)`: no `restarting` line, so nothing reopened during the 20 s of silence |
| 2 | Read `"$P/meta.json"` | Both tracks have `frames` ÷ `sample_rate` within 1.0 s of each other; no event of kind `write_failed` |
| 3 | `CALL="$(python3 tests/fixtures/recordings.py copy long-silence --title "REC-017 silence")"; python3 -c "import json,sys;[print(round(s['start'],1),s['speaker'],s['text']) for s in json.load(open(sys.argv[1]+'/segments.json'))]" "$CALL"` | Three or more segments; none starts between 6 s and 25 s (the silence: Dana's line starts at about 26 s, its first segment may start a few tenths earlier); the speakers are Me, Remote, Me in that order |

## Expected Final State
The silent stretch is in the tracks as silence, with no restart of a source, and the transcript has nothing there. (The score of the same recording, words and silence, is `transcription/TC-005`.)

## Notes
- Run on 2026-11-07 (S-18, its own capture then): PASS — WER 0.0, nothing in the silence, the only `restarting` at 40.0 s (TestCall's hang-up). `prepare` ends the capture before the hang-up, so there is none now.
- 2026-11-08: the prepared capture is fed from files: its sources never restart, so step 1 shows the recorder kept both tracks through the silence (padded, aligned), not how a real tap behaves in one; a real device's silence is the live check's (`privacy/TC-002`, `recording/TC-023`).
