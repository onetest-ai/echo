---
id: TC-012
title: A Ukrainian call started as English and changed to Ukrainian once both sides were transcribed in English comes out in Ukrainian, all of it, the mic included
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
| Conversation | `ukrainian-call` (3 turns, Ukrainian: Me, Olena, Me; no echo — as on headphones) |
| Arm | title "TRN-012 relanguage", language `en` (the wrong one) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en,uk --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app` | The app runs on the test data |
| 2 | Run the prelude, then `date +%s > "$D/ev.start"; target/debug/echoctl arm --title "TRN-012 relanguage" --languages en; $C start ukrainian-call; $W 20 recording $T; CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; grep '"language"' "$CALL/meta.json"; for i in $(seq 60); do python3 -c "import json,sys;l=[json.loads(x) for x in open(sys.argv[1])];sys.exit(0 if {x['speaker'] for x in l}>={'me','remote'} else 1)" "$CALL/live.jsonl" 2>/dev/null && break; sleep 0.5; done; python3 -c "import json,sys;[print(x['speaker'],x['lang']) for x in map(json.loads,open(sys.argv[1]))]" "$CALL/live.jsonl"; target/debug/echoctl language uk` — in one command | `meta.json` said `"language": "en"`; at least one `me en` and one `remote en` line were decoded before the change; then "…: uk from now on; the whole call is transcribed again in uk when it ends" |
| 3 | Run the prelude, then `$W 120 event transcribed; grep -E "  model:|language: uk \(changed during the call\)|transcribing all of it again" "$ECHOCTL_HOME/.echo/app.log"` | In order: "model: parakeet-tdt-0.6b-v3" (an English call), "language: uk (changed during the call)", "model: large-v3-turbo-q5_0 (for uk)" (the model follows the new language), "the call's language was changed during it: transcribing all of it again" |
| 4 | Run the prelude, then `grep '"language"' "$CALL/meta.json"; cp "$D/conv/ukrainian-call/expected.json" "$CALL/"; python3 tests/fixtures/audio/make_audio.py score "$CALL"` | `"language": "uk"`; exit 0, `"pass": true`, `wer` at most 0.15, all three turns `lang_ok` (uk) and `speaker_ok`: the Me lines decoded in English before the change are Ukrainian too |
| 5 | Run the prelude, then `target/debug/echoctl language en "$CALL"` | Refused: "its audio was deleted once the transcript was written, so it can't be transcribed again in another language"; `meta.json` still says `uk` |

## Expected Final State
A call recorded in the wrong language and changed during it — after both sides were already transcribed in it — comes out wholly in the new language, both tracks, with the model for it; after the transcript, with the audio gone, its language can't be changed.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when the call hung up by itself)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- No echo here (the headphones case): the mic is decoded again whole, not through echo cancellation, which finds none and would leave its English lines.
- A change made just as the call ends is caught before the transcript is written (`watch::finish_transcript`): the call is transcribed again from its audio in the new language.
- The window changes it the same way (`call_language::change`, the `set_language` action); `echoctl language` names the latest call by default, the one recording.
