---
id: TC-018
title: Two voices on the remote track stay Remote and are told apart
priority: medium
type: functional
requirements: [F-STT-04]
module: recording
run: agent
platform: macos
tags: [recording, test-call, speakers, recordings]
---

# TC-018: Two voices on the remote track stay Remote and are told apart

**Module:** Recording | **Priority:** Medium | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"): `two-remotes` (Dana and Omar on the remote side, one track; the user on the mic; 34 s) was played through TestCall, captured and transcribed once with NVIDIA's NeMo-Speech runtime in the config (`prepare` fetches it into `target/test-recordings/nemo-speech`), so speaker detection ran. This case reads that result; it records and transcribes nothing
- Every command after `. <dir>/env` on test data built by `tests/fixtures/make-data.sh <dir>`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `CALL="$(python3 tests/fixtures/recordings.py copy two-remotes --title "REC-018 two voices")"; python3 -c "import json,sys;[print(round(s['start'],1),s['speaker'],s['text']) for s in json.load(open(sys.argv[1]+'/segments.json'))]" "$CALL"` | Every line of Dana's and Omar's is on the remote side (not `me`); the user's lines are `me` |
| 2 | `target/debug/echoctl speakers "$CALL"` | It lists two remote speakers (Speaker 1 and Speaker 2); the first lines of one are Dana's ("Hi everyone, Omar is joining us…"), of the other Omar's ("Hello. I have three questions…") |
| 3 | Read `"$CALL/transcript.md"` | The remote lines of the two voices carry two different speaker labels; the user's lines are Me |

## Expected Final State
One remote track, two voices: both are Remote, and the transcript tells them apart as two speakers. (The words of this recording are scored in `transcription/TC-003`.)

## Notes
- Run on 2026-11-07 (S-18, its own capture then): PASS with the runtime fetched: Speaker 1 = Dana's lines, Speaker 2 = Omar's; WER 0.029. Without it: "no detected speakers".
- If the detector merges the two voices into one speaker, step 2 fails: that is what this case checks (F-STT-04).
