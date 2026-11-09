---
id: TC-012
title: A Russian call started as English and changed to Russian during it comes out in Russian, all of it
priority: high
type: functional
requirements: [F-STT-09, F-STT-08, F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, file-fed, second-instance]
---

# TC-012: A call's language is changed during it

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `transcription/TC-010`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, the speech model linked, a second app instance on the test data (it starts in step 1). The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=trn-012-relanguage`
- The speech models and the NeMo-Speech runtime linked into the run's config as in `transcription/TC-011` (`parakeet-tdt-0.6b-v3` and `large-v3-turbo-q5_0` downloaded), `large-v3-turbo-q5_0` in use
- No call is on, nothing armed or recording; no other app instance on this data

## Test Data

| Field | Value |
|-------|-------|
| Conversation | `russian-call` (2 turns, Russian) |
| Arm | title "TRN-012 relanguage", language `en` (the wrong one) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en,ru --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app` | The app runs on the test data |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "TRN-012 relanguage" --languages en; $C start russian-call; $W 20 recording $T; grep '"language"' "$(ls -d "$ECHOCTL_HOME"/calls/*-$T)/meta.json"; target/debug/echoctl language ru` — at once, in one command: the call is about 10 s long | The recording runs, its `meta.json` said `"language": "en"`; then "…: ru from now on; the whole call is transcribed again in ru when it ends" |
| 3 | Run the prelude, then `$W 120 event transcribed; grep -E "  model:|language: ru \(changed during the call\)|transcribing all of it again" "$ECHOCTL_HOME/.echo/app.log"` | In order: "model: parakeet-tdt-0.6b-v3" (an English call), "language: ru (changed during the call)", "model: large-v3-turbo-q5_0 (for ru)" (the model follows the new language), "the call's language was changed during it: transcribing all of it again" |
| 4 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; grep '"language"' "$CALL/meta.json"; cp "$D/conv/russian-call/expected.json" "$CALL/"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | `"language": "ru"`; exit 0, `"pass": true`, `wer` at most 0.15, both turns `lang_ok` (ru): no English line left from before the change |
| 5 | Run the prelude, then `target/debug/echoctl language uk "$CALL"` | Refused: "its audio was deleted once the transcript was written, so it can't be transcribed again in another language"; `meta.json` still says `ru` |

## Expected Final State
A call recorded in the wrong language and changed during it is transcribed in the new language from then on and all of it again at its end; after the transcript, with the audio gone, its language can't be changed.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when the call hung up by itself)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- The window changes it the same way (`call_language::change`, the `set_language` action); `echoctl language` names the latest call by default, the one recording.
