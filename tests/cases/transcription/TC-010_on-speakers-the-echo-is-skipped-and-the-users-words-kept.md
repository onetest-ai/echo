---
id: TC-010
title: On speakers the echo is found during the call and not transcribed, and the user's words over the others are
priority: high
type: functional
requirements: [F-STT-07, F-STT-03]
module: transcription
run: agent
platform: macos
tags: [transcription, echo, second-instance, file-fed]
---

# TC-010: On speakers the echo is skipped during the call and the user's words are kept

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `recording/TC-025`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, the speech model linked, a second app instance on the test data (it starts in step 1). The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=trn-010-speakers`
- No call is on, nothing armed or recording; no other app instance on this data (in S-18: after stage 11's teardown quit the prelude's)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | `speakers-echo` (126 s, 24 turns): acme-sync and two-remotes twice; the mic hears the other side 0.35× and 40 ms late (`"echo"` in `conversations.json`); from 79 s the user talks over them four times ("I'll send the checklist…" at 79.6 s, "Let's agree on Berlin first…" 87.7 s, "Go ahead, Omar…" 107.6 s, "Yes, the audio is deleted…" 116.3 s) |
| Arm title | TRN-010 speakers |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"`, `target/debug/echoctl arm --title "TRN-010 speakers"`, `$W 10 armed` | "waited … for app", then "waited … for armed" |
| 2 | Run the prelude, then `$C start speakers-echo; $W 20 recording $T` | The recording runs (the call hangs up by itself after 127 s) |
| 3 | Run the prelude, then `$W 240 event transcribed; grep -E '^  echo' "$ECHOCTL_HOME/.echo/app.log"; grep -c 'echo cancellation: on' "$ECHOCTL_HOME/.echo/app.log"` | Two lines: "echo found N s into the call: 30 ms, gain G, r R — …" with N between 45 and 90, G between 0.3 and 0.4, R at least 0.8; then "echo: 30 ms, gain G, r R; skipped S s of the mic" with S at least 20. The count is 0: the mic isn't decoded again at the end |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; cp "$D/conv/speakers-echo/expected.json" "$CALL/"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | Exit 0, `"pass": true` (top-level `wer` at most 0.15); every turn `speaker_ok` true: none of Dana's or Omar's lines came out as Me. The four lines said over the others (`start` 79.57, 87.66, 107.56, 116.28) each have a `got` and a `wer` at most 0.2 |

## Expected Final State
The echo of the others was found during the call and its stretches of the mic weren't transcribed; the user's own lines, also those said over the others, are in the transcript as Me; the mic wasn't decoded a second time.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when the call hung up by itself)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- The echo is made, not heard: `make_audio.py build` adds the remote track to the mic as the conversation's `echo` says. A real room (its colouring, reverberation, noise) is `recording/TC-008`, person-run.
- Before the echo is found (about 30 s of the others talking) the mic is transcribed whole; the lines of the others it repeats as Me then are dropped by the transcript's echo filter ("dropped N mic segment(s) that echoed the remote side" in `app.log`).
- The call's own transcription is the app's (the watcher's live worker): `echoctl transcribe` on the same tracks takes another path (the whole mic echo-cancelled, then decoded), not what this case checks.
