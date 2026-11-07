---
id: TC-004
title: A language pinned for one call applies to that call only
priority: medium
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, test-call]
external_id: TC-TRN-005
---

# TC-004: A language pinned for one call applies to that call only

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- This Mac has the test call app (`tests/tools/testcall/build.sh` built `tests/tools/testcall/build/TestCall.app`), BlackHole 2ch and ffmpeg; Microphone and System Audio Recording are granted to the process running the steps and to TestCall; nothing else on this Mac plays audio or uses BlackHole
- Every Bash call starts with the prelude (a fresh shell each time; the steps below call it "the prelude"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; CONV="$D/conv"; APP="$PWD/tests/tools/testcall/build/TestCall.app/Contents/MacOS/testcall"; export D CONV APP ECHOCTL_CALL_APP_BUNDLE=ai.onetest.echo.testcall`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0` and `target/debug/echoctl config --languages en`
- Both ends of the call run on this Mac: `serve` and `dial` on `127.0.0.1`, as in the steps
- Settings → Languages has three: run `target/debug/echoctl config --languages en,uk,ru` after the prelude

## Test Data

| Field | Value |
|-------|-------|
| Pinned call | mixed-languages: Ukrainian, Russian and English lines (31 s), recorded with `--languages uk` |
| Next call | very-short (9 s), no pin |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build mixed-languages "$CONV"` | It prints the folder, the length and the turns of mixed-languages; `$CONV` holds `mic.wav`, `remote.wav` and `expected.json` |
| 2 | Run the prelude, then `target/debug/echoctl config --languages en,uk,ru` | The printed config has `languages` ["en", "uk", "ru"] |
| 3 | Run the prelude, then `"$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 38 --languages uk --title "TRN-004 pinned" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is the call folder `…-trn-004-pinned` |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-trn-004-pinned)"; grep -A1 languages "$CALL/meta.json"` | `meta.json` has `languages` ["uk"] |
| 5 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-trn-004-pinned)"; target/debug/echoctl transcribe "$CALL"; grep -o '"lang": *"[a-z]*"' "$CALL/segments.json" | sort | uniq -c` | "done in …"; one language value only: `uk`, on every segment (no `en`, no `ru`) |
| 6 | Run the prelude, then `target/debug/echoctl config` | `languages` is still ["en", "uk", "ru"]: the pin did not change the workspace setting |
| 7 | Run the prelude, then `python3 tests/fixtures/audio/make_audio.py build very-short "$D/conv2"; CONV="$D/conv2"; "$APP" serve --port 7792 --play "$CONV/remote.wav" --quiet & "$APP" dial 127.0.0.1:7792 --me "$CONV/mic.wav" --me-device "BlackHole 2ch" & sleep 2; target/debug/echoctl capture --seconds 16 --title "TRN-004 next" 2> "$D/capture.err"; wait` | Exit code 0; the last line printed is the call folder `…-trn-004-next` |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-trn-004-next)"; grep -c languages "$CALL/meta.json"; target/debug/echoctl transcribe "$CALL"; grep -o '"lang": *"[a-z]*"' "$CALL/segments.json" | sort | uniq -c` | The count is 0 (no pin in this call's `meta.json`); the segments are `en`: the next call uses the workspace languages |

## Expected Final State
A language pinned for one call applies to that call only; the workspace languages and the next call are untouched.

## Teardown
- Run the prelude, then `target/debug/echoctl config --languages en`

## Notes
- Run on 2026-11-07 (S-17): PASS but for `capture`'s exit code (134 after saving, see `recording/TC-014`): 8 segments all `uk`; the next call `en`; workspace languages untouched.
- The pin is `capture --languages`; the record menu's "Only Ukrainian" and `echoctl arm --languages uk` write the same `languages` into `meta.json`.
