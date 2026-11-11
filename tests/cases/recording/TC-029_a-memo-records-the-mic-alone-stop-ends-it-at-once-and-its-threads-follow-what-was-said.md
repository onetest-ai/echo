---
id: TC-029
title: A memo started from the pill records the mic alone; its Stop ends it without asking; the minutes run is given the open threads and the split it returns pins the memo to them
priority: high
type: functional
requirements: [F-REC-06]
module: recording
run: agent
platform: macos
tags: [recording, memo, second-instance, file-fed]
---

# TC-029: A memo records the mic alone, Stop ends it without asking, its threads follow what was said

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set (threads Acme rollout and Globex pilot), hermetic — the app this case starts runs Claude Code on its own and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- The mic is fed from a file (app_profile.md → "Recording with file-fed calls"): the memo's mic source is the debug build's test source, `ECHOCTL_TEST_ME_WAV`; no device is opened, no sound played, no permission needed. With no call file the file plays from the moment the memo opens it
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- **Needs a second app instance** (the watcher starts the memo), as in `recording/TC-003`; if `$D/app.out` says "already running" the case is BLOCKED, not failed
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`tests/fixtures/serve.sh up`); the page is loaded from `{{base_url}}/` first
- No call is on and nothing is armed or recording
- **This case writes one file in the run's data folder** (step 9): `memo-threads.new.json` in the memo's own folder, the file the minutes run writes and the only one `echoctl memo threads` reads. The fake Claude Code refuses the minutes run that would write it. The step is the one exception to the runner's read-only Bash, on the run's own data only

## Test Data

| Field | Value |
|-------|-------|
| Feed | acme-sync's tracks put at the feed paths by `call.py start acme-sync --hold` and then `call.py end`: `mic.wav` holds only Sam's lines ("Morning Dana, let's go through the single sign-on rollout.", "I'll send the checklist this afternoon, before four o'clock.", "Let's agree on Berlin first. Madrid follows two weeks later."), `remote.wav` Dana's ("…ideally by the end of today.", "…the Berlin office or with Madrid?", "…I'll book the training room for Thursday."). The memo hears the mic only |
| Memo | started from the Memo button, stopped about 22 s after it started (`sleep 22`: the feed's own timeline) |
| Split (step 9) | `[{"thread":"acme-rollout","from":0,"to":6},{"thread":"globex-pilot","from":6,"to":12}]` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `$C start acme-sync --hold; $C end` (puts the tracks at the feed paths; no app is running yet), `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; `call.py end` prints "hung up"; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null and `armed` null |
| 2 | Run the prelude, then `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"`; load `{{base_url}}/`, wait for "Your day"; read the top bar's pill | The idle pill: "Record next call" and a button "Record a memo" (`[data-cmd="record-memo"]`, visible text "Memo") |
| 3 | Click "Record a memo"; run the prelude, then `$W 20 recording memo; target/debug/echoctl status` | "waited … for recording memo"; `watcher.recording` is a folder ending `-memo` |
| 4 | Snapshot the top bar | A group "Recording a memo" (`.pill.rec`) with a microphone mark (no red dot), a running clock, a button "Memo" (title "Open the memo") and a button "Stop" (`[data-cmd="stop"]`); no language picker and no "Stop recording?" |
| 5 | Run the prelude, then `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; ls "$M"; grep -c '"kind": *"memo"' "$M/meta.json"; grep -c '"remote' "$M/meta.json"` | The folder holds `mic.wav` and `meta.json` and **no `remote.wav`**; the first count is 1 (`"kind": "memo"`); the second is 0: no remote track |
| 6 | Run the prelude, then `sleep 22` (the memo's timeline: Sam's third line has been said), and click "Stop" in the pill (`[data-cmd="stop"]`) at once | The pill does not ask "Stop recording?" and shows no "Keep recording": the click ends the memo |
| 7 | Run the prelude, then `$W 10 not-recording; target/debug/echoctl status; $W 120 event transcribed` | `watcher.recording` is null and `watcher.in_call` is not true; "waited … for event transcribed"; the pill is the idle one again ("Record next call", "Memo") |
| 8 | Run the prelude, then `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; ls "$M"; grep -o '"stop_reason": *"[a-z_]*"' "$M/meta.json"; grep -c "this afternoon" "$M/transcript.md"; grep -c -e "end of today" -e "Berlin office" -e "training room" "$M/transcript.md"; target/debug/echoctl context "$M" > "$D/ctx.json"; grep -c '"memo": *true' "$D/ctx.json"; grep -c "acme-rollout" "$D/ctx.json"; grep -c "globex-pilot" "$D/ctx.json"` | `transcript.md` and `segments.json` are there and no `.wav`; `"stop_reason": "user"`; "this afternoon" 1 or more (Sam's line, from the mic); the second count is **0** (nothing Dana said was recorded); in the context the minutes run gets, `"memo": true` (1 or more) and the open threads named: `acme-rollout` and `globex-pilot` each 1 or more |
| 9 | Run the prelude, then `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; printf '[{"thread":"acme-rollout","from":0,"to":6},{"thread":"globex-pilot","from":6,"to":12}]\n' > "$M/memo-threads.new.json"; target/debug/echoctl memo threads "$M" --file "$M/memo-threads.new.json"; target/debug/echoctl project of "$M"` | `memo threads` prints `parts` with acme-rollout 0–6 and globex-pilot 6–12 and `dropped` empty; `project of` lists acme-rollout and globex-pilot as the memo's threads |
| 10 | Reload `{{base_url}}/`, wait for "Your day", navigate to `{{base_url}}/#calls`, click the row `[data-call$="-memo"]` | The header says "Memo" and has two thread chips: "Acme rollout" followed by "0:00–0:06" and "Globex pilot" followed by "0:06–0:12"; the chip's title is "Found from what you said" |

## Expected Final State
A memo recorded Sam's mic only (no remote track, nothing of Dana's lines in its transcript), the pill's Stop ended it without a question, and the threads it was pinned to are the ones the split named, shown as chips with their spans.

## Teardown
- Run the prelude, then `$C end` (prints "no call")
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- The memo's minutes (and so its thread split) come from a Claude Code run the app starts as soon as the memo is transcribed (`notes::auto`, no wait for a memo). On this data the fake `claude` refuses it, so no minutes are written and an Inbox item may say the run failed: not a failure of this case. Step 9 writes by hand the split the run would hand to `echoctl memo threads`; what the model decides from the words is `claude/TC-017` (the local model) and the chips' behaviour `calls/TC-015`.
- `Recording::start_memo` opens the default input (`audio::mic_source(None)`) and never the call app's audio: step 5's missing `remote.wav` is that.
- A memo through `echoctl memo start` / `echoctl memo stop` and the menu bar's entry is the same recording; the pill's Memo and Stop are what is checked here. The real microphone is `privacy/TC-002`'s kind of check (person).
- Written 2026-11-11 from the code (`10-shell.js` `pill`, `90-render.js` `stop-ask`, `recorder.rs` `start_memo`, `memo.rs` `set_threads`); not yet run.
