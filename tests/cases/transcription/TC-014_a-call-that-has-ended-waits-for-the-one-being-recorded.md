---
id: TC-014
title: The rest of a call that has ended, decoded with Whisper, waits while the next recording goes on and is finished once it stops
priority: high
type: functional
requirements: [F-STT-11]
module: transcription
run: agent
platform: macos
tags: [transcription, back-to-back, file-fed, second-instance]
---

# TC-014: A call that has ended gives the next recording the computer first

**Module:** Transcription | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `transcription/TC-012`: test data from `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up … --ai fake`), calls fed from files, the debug build, the speech models and the NeMo-Speech runtime linked into the run's config as in `transcription/TC-011` (`parakeet-tdt-0.6b-v3` and `large-v3-turbo-q5_0` downloaded), `large-v3-turbo-q5_0` in use. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D; T=trn-014-first`
- No call is on, nothing armed or recording; no other app instance on this data

## Test Data

| Field | Value |
|-------|-------|
| Conversation | `speakers-echo` (126 s, both sides talking, on speakers) |
| Arm | title "TRN-014 first", language `en`, changed to `uk` during the call: its end decodes the whole call again with Whisper — minutes of work left after the call |
| The next recording | a memo (the mic only), started as soon as the first call's recording stops — as the next call of a back-to-back day |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --languages en,uk --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app` | The app runs on the test data |
| 2 | Run the prelude, then `target/debug/echoctl arm --title "TRN-014 first" --languages en; $C start speakers-echo; $W 20 recording $T; CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-$T)"; echo "$CALL" > "$D/call"; for i in $(seq 60); do python3 -c "import json,sys;l=[json.loads(x) for x in open(sys.argv[1])];sys.exit(0 if {x['speaker'] for x in l}>={'me','remote'} else 1)" "$CALL/live.jsonl" 2>/dev/null && break; sleep 0.5; done; target/debug/echoctl language uk` — in one command | Recording "TRN-014 first"; "…: uk from now on; the whole call is transcribed again in uk when it ends" |
| 3 | Run the prelude, then `wc -l < "$ECHOCTL_HOME/.echo/app.log" > "$D/log.mark"; $W 200 event recording_stopped; target/debug/echoctl memo start; $W 20 recording; sleep 20; CALL="$(cat "$D/call")"; ls "$CALL"; tail -n +"$(( $(cat "$D/log.mark") + 1 ))" "$ECHOCTL_HOME/.echo/app.log" \| grep -c "the rest of this one waits until it ends"` — in one command, the memo started at once | The memo records; 20 s later the first call still has **no** `transcript.md` (its WAVs kept); the count is `1`: said once, for its whole rest |
| 4 | Run the prelude, then `target/debug/echoctl memo stop; $W 300 sh '[ -f "$(cat "$D/call")/transcript.md" ]'; CALL="$(cat "$D/call")"; M="$(ls -td "$ECHOCTL_HOME"/calls/*-memo/ \| head -1)"; python3 -c "import json,os,sys,datetime as d; e=d.datetime.fromisoformat(json.load(open(sys.argv[2]+'/meta.json'))['ended_at']).timestamp(); t=os.path.getmtime(sys.argv[1]+'/transcript.md'); print('after the memo' if t>e else 'BEFORE the memo')" "$CALL" "$M"; grep '"language"' "$CALL/meta.json"; grep -c . "$CALL/transcript.md"` | Once the memo stops, the first call's transcript is written: "after the memo"; `"language": "uk"`; the transcript has lines (both sides) |

## Expected Final State
A call whose end still had Whisper's work to do waited while the next recording went on, and was finished once it stopped.

## Teardown
- Run the prelude, then `$C end` (prints "no call" when the call hung up by itself); `target/debug/echoctl memo stop` (refused when none records)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Written from the feature's text (F-STT-11), not from the code. Found by the user (#903): on a slower Mac, sharing a window in Teams failed ("Couldn't share your content") while Echo finished the previous call's transcript at the start of the next one; reproduced with the processor or the GPU kept busy, never with Echo reading Teams' window alone.
- The transcript's quality after a change of language is `transcription/TC-012`'s; here the English conversation decoded as Ukrainian only makes minutes of work after the call.
- Parakeet's or Nemotron's rest and speaker detection aren't held until the end: they go on between the utterances of the call being recorded — `live::tests::a_finishing_call_decodes_once_the_utterance_being_decoded_is_done`; one copy of a model for two calls: `transcribe::tests::a_model_is_loaded_once_while_held_and_again_after_it_was_let_go`.
