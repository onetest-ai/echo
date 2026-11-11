---
id: TC-029
title: A memo started from the pill records the mic alone; its Stop ends it without asking; its threads come from what was said (the minutes run's split, on the virtual model) and show as chips
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
- Test data built by `tests/fixtures/make_data.py <dir> --ai virtual` (or `serve.sh up <dir> <port> --ai virtual`; `tests/fixtures/README.md`, "The virtual model"): the small set (threads Acme rollout and Globex pilot), hermetic — the app's minutes run is real Claude Code answered by the scripted model, no real account or model; Claude Code on `PATH`; `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- The mic is fed from a file (app_profile.md → "Recording with file-fed calls"): the memo's mic source is the debug build's test source, `ECHOCTL_TEST_ME_WAV`; no device is opened, no sound played, no permission needed. With no call file the file plays from the moment the memo opens it
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- The speech model is linked into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, then `target/debug/echoctl model use large-v3-turbo-q5_0`
- **Needs a second app instance** (the watcher starts the memo), as in `recording/TC-003`; if `$D/app.out` says "already running" the case is BLOCKED, not failed
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`tests/fixtures/serve.sh up`); the page is loaded from `{{base_url}}/` first
- No call is on and nothing is armed or recording
- **The model is a script.** Step 1 writes `$D/model/model-script.json`, the virtual model's answers for a memo's minutes run: it reads the context and the transcript, writes the split of the memo into the threads it is about and hands it to `echoctl memo threads` (the steps `plugin/skills/process-call` gives a memo), writes the minutes. That is the one file the case writes by hand, and it stands in for the model's judgment: what this case proves is that the app's own minutes run offers the open threads, takes the split and shows it. Whether a real model splits by what was said is the person-run half (Notes)

## Test Data

| Field | Value |
|-------|-------|
| Feed | acme-sync's tracks put at the feed paths by `call.py start acme-sync --hold` and then `call.py end`: `mic.wav` holds only Sam's lines ("Morning Dana, let's go through the single sign-on rollout.", "I'll send the checklist this afternoon, before four o'clock.", "Let's agree on Berlin first. Madrid follows two weeks later."), `remote.wav` Dana's ("…ideally by the end of today.", "…the Berlin office or with Madrid?", "…I'll book the training room for Thursday."). The memo hears the mic only |
| Memo | started from the Memo button, stopped about 22 s after it started (`sleep 22`: the feed's own timeline) |
| The script's split | acme-rollout 00:00–00:06, globex-pilot 00:06–00:12 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then write the script: `cat > "$D/model/model-script.json" <<'EOF'` with `{"model": {"context_length": 32768}, "rules": [{"when": ["/echo:process-call"], "vars": {"dir": "<command-args>(\\S+)", "actions": "write the new action list to `([^`]+)`"}, "steps": [{"tool": "Bash", "input": {"command": "echoctl context {dir}", "description": "Read the call's context"}}, {"tool": "Read", "input": {"file_path": "{dir}/transcript.md"}}, {"tool": "Write", "input": {"file_path": "{dir}/memo-threads.new.json", "content": "[{\"thread\":\"acme-rollout\",\"from\":\"00:00\",\"to\":\"00:06\"},{\"thread\":\"globex-pilot\",\"from\":\"00:06\",\"to\":\"00:12\"}]"}}, {"tool": "Bash", "input": {"command": "echoctl memo threads {dir} --file {dir}/memo-threads.new.json", "description": "Pin the memo to its threads"}}, {"tool": "Write", "input": {"file_path": "{dir}/notes.md", "content": "# Memo\n\n## Summary\n- Two things said, one per thread.\n"}}, {"tool": "Write", "input": {"file_path": "{actions}", "content": "[]"}}, {"tool": "Bash", "input": {"command": "echoctl actions {dir} --apply {actions}", "description": "Apply the action items"}}, {"text": "Memo minutes written (virtual model)."}]}]}` and `EOF`; then `$C start acme-sync --hold; $C end` (puts the tracks at the feed paths; no app is running yet), `target/debug/echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; `call.py end` prints "hung up"; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null and `armed` null |
| 2 | Load `{{base_url}}/`, wait for "Your day"; read the top bar's pill | The idle pill: "Record next call" and a button "Record a memo" (`[data-cmd="record-memo"]`, visible text "Memo") |
| 3 | Click "Record a memo"; run the prelude, then `$W 20 recording memo; target/debug/echoctl status` | "waited … for recording memo"; `watcher.recording` is a folder ending `-memo` |
| 4 | Snapshot the top bar | A group "Recording a memo" (`.pill.rec`) with a microphone mark (no red dot), a running clock, a button "Memo" (title "Open the memo") and a button "Stop" (`[data-cmd="stop"]`); no language picker and no "Stop recording?" |
| 5 | Run the prelude, then `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; ls "$M"; grep -c '"kind": *"memo"' "$M/meta.json"; grep -c '"remote' "$M/meta.json"` | The folder holds `mic.wav` and `meta.json` and **no `remote.wav`**; the first count is 1 (`"kind": "memo"`); the second is 0: no remote track |
| 6 | Run the prelude, then `sleep 22` (the memo's timeline: Sam's third line has been said), and click "Stop" in the pill (`[data-cmd="stop"]`) at once | The pill does not ask "Stop recording?" and shows no "Keep recording": the click ends the memo |
| 7 | Run the prelude, then `$W 10 not-recording; target/debug/echoctl status; $W 120 event transcribed` | `watcher.recording` is null and `watcher.in_call` is not true; "waited … for event transcribed"; the pill is the idle one again ("Record next call", "Memo") |
| 8 | Run the prelude, then `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; ls "$M"; grep -o '"stop_reason": *"[a-z_]*"' "$M/meta.json"; grep -c "this afternoon" "$M/transcript.md"; grep -c -e "end of today" -e "Berlin office" -e "training room" "$M/transcript.md"` | `transcript.md` and `segments.json` are there and no `.wav`; `"stop_reason": "user"`; "this afternoon" 1 or more (Sam's line, from the mic); the second count is **0** (nothing Dana said was recorded) |
| 9 | Run the prelude, then the user's Write minutes from the terminal: `M="$(ls -d "$ECHOCTL_HOME"/calls/*-memo)"; target/debug/echoctl notes "$M" --wait; ls "$M"; grep -c "echoctl memo threads" "$M/claude.log"; target/debug/echoctl project of "$M"` | `Minutes written: …/notes.md`; `claude.log` shows the run called `echoctl memo threads`; `project of` lists acme-rollout and globex-pilot as the memo's threads (taken from what was said, by the minutes run) |
| 10 | Reload `{{base_url}}/`, wait for "Your day", navigate to `{{base_url}}/#calls`, click the row `[data-call$="-memo"]` | The header says "Memo" and has two thread chips: "Acme rollout" followed by "0:00–0:06" and "Globex pilot" followed by "0:06–0:12"; the chip's title is "Found from what you said" |

## Expected Final State
A memo recorded Sam's mic only (no remote track, nothing of Dana's lines in its transcript), the pill's Stop ended it without a question, and the app's own minutes run split it into the threads the (scripted) model named, shown as chips with their spans.

## Teardown
- Run the prelude, then `$C end` (prints "no call")
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- **Person-run half (not run by an agent): that the threads follow what was said.** The virtual model answers from a script, so this case shows the plumbing, not the judgment. A person runs it on `--ai local` (or a real Claude Code): record a memo, say two sentences about Acme rollout then two about Globex pilot, Stop; after the minutes the header holds the chips "Acme rollout" and "Globex pilot" in that order, each over the stretch where it was talked about, and a stretch about neither is left out. The model's side is also `claude/TC-017`; the chips' behaviour `calls/TC-015`.
- A memo through `echoctl memo start` / `echoctl memo stop` and the menu bar's entry is the same recording; the pill's Memo and Stop are what is checked here. The real microphone is `privacy/TC-002`'s kind of check (person).
- The mic is fed from a file (the debug build's test source), so no device or permission is involved.
- Rewritten 2026-11-11 from F-REC-06 and the README (the earlier draft wrote the split file by hand); not yet run. Step 1's script is the one hand-written file; the split file itself is written by the minutes run.
- Step 9 starts the minutes as the user does (Write minutes): a memo this short is below the automatic minutes' length gate, and the prelude turns automatic minutes off.
