---
id: TC-001
title: Record the next call: armed, then recorded from connect to hang-up
priority: critical
type: functional
requirements: [F-REC-02, F-REC-08, F-REC-13]
module: recording
run: agent
platform: macos
tags: [recording, arm, second-instance]
external_id: TC-REC-001
---

# TC-001: Record the next call: armed, then recorded from connect to hang-up

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`): the small set, hermetic — the app this case starts runs Claude Code on its own (preps, meeting types, who does a task) and must reach no real account — and its `<dir>/env` points the debug build's mic, call-app audio and call detection at files (`app_profile.md` → "Recording with file-fed calls": no device opened, no sound played, no permission needed); `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Every Bash call starts with the prelude (a fresh shell each time; the steps below and other cases call it "the prelude of `recording/TC-001`"): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`. `$C start <conversation>` starts a call fed from the conversation's files (it hangs up by itself 1 s after the conversation; `--hold` keeps it on until `$C end`); `$W <seconds> <condition>` waits for the app's state or an event, with a timeout (`tests/fixtures/wait.py`), instead of a fixed `sleep`
- **Needs a second app instance.** Echo's watcher (arm, record when the call starts, stop) runs only inside `echoctl app run`. The user's own Echo holds `app.lock` in its own config folder; this run's `ECHOCTL_CONFIG_DIR` has another one (`app.rs`, `try_lock`), so a second instance on the test data folder is not refused by the lock. If step 2 still finds "Echo is already running" in `$D/app.out`, the case is BLOCKED, not failed
- **The main call.** In S-17 the app, the arm and the call are the scenario's shared assets: its prelude runs steps 1–4 once (the call with `--hold`), and window/TC-014 hangs the call up; this stage goes on from step 5. On its own: steps 1–4 here (the call hangs up by itself at 37 s)

## Test Data

| Field | Value |
|-------|-------|
| Conversation | acme-sync (36 s), fed from files |
| Arm title | REC-001 next call |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then link the speech model into the test config: `ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, `target/debug/echoctl model use large-v3-turbo-q5_0`, `target/debug/echoctl config --languages en` | "active model: large-v3-turbo-q5_0"; the config call exits 0 |
| 2 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `cat "$ECHOCTL_HOME/.echo/events.jsonl" 2>/dev/null \| wc -l > "$D/ev.start"` (the events before this case), `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app". `status` prints `watcher` with a `pid`, `app` "teams", `recording` null and `armed` false; `armed` is null; no "already running" in `$D/app.out` |
| 3 | Run the prelude, then `target/debug/echoctl arm --title "REC-001 next call"`, `$W 10 armed` (the watcher takes the arm at its next tick), then `target/debug/echoctl status` | arm prints "armed — recording starts when the call begins (expires in 15 min)"; `status` has `armed` with `title` "REC-001 next call" and `watcher.armed` true |
| 4 | Run the prelude, then `$C start acme-sync` (in S-17: `$C start acme-sync --hold`), `$W 20 recording rec-001-next-call; target/debug/echoctl status`; then, while it records, list its folder twice 2 s apart: `{ ls -l "$ECHOCTL_HOME"/calls/*-rec-001-next-call; sleep 2; ls -l "$ECHOCTL_HOME"/calls/*-rec-001-next-call; } > "$D/rec-001.during"` (the evidence `recording/TC-014` and `privacy/TC-003` read) | `call.py` prints the conversation and `started_at`; within 20 s `status` shows `watcher.in_call` true, `watcher.recording` a folder path ending `-rec-001-next-call`, and `armed` null (the arm is used up); `$D/rec-001.during` holds two listings |
| 5 | Run the prelude, then `$W 60 event recording_stopped; tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" \| grep -o '"event":"[a-z_]*"'` | In order: `call_started`, `recording_started`, `call_ended`, `recording_stopped` (10–15 s after the hang-up: the watcher's tick, 2 s for the mic and 10 s of grace), then `transcribed` |
| 6 | Run the prelude, then `$W 120 file "$ECHOCTL_HOME/calls/*-rec-001-next-call/transcript.md"; ls "$ECHOCTL_HOME"/calls/*-rec-001-next-call` | The folder has `transcript.md` and `segments.json`, and no `.wav` file (the app transcribed it and deleted the audio) |
| 7 | Read `meta.json` of that folder | `stop_reason` is "call_ended", `ended_at` is set, `title` is "REC-001 next call", `tracks` lists `mic.wav` and `remote.wav` |
| 8 | Run the prelude, then `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-001-next-call)"; grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: the last two turns ("Berlin first", "training room") are in the transcript; the first turn may be clipped by the second the watcher needs to notice the call, and is not asserted |

## Expected Final State
An armed Echo recorded the next call from its connect to its hang-up, closed it with reason "call_ended" and transcribed it, with no audio left behind.

## Teardown
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`. In S-17: skipped (the app stays for the later stages; `claude/TC-010` quits it)
- Run `target/debug/echoctl status`: `watcher` is `null`

## Notes
- Since 2026-11-08 evening the call is fed from files (no TestCall, BlackHole or permission): everything after the sources is Echo's real path. The mic falling back to the room after the hang-up (#140) can't happen with file sources; it is the live check's (`recording/TC-023`, person).
- Run 2026-11-08 evening (S-17, CLI part, file-fed, the call held 27 s and hung up): events in order, `recording_stopped` 13 s after the hang-up, `transcribed` 1 s later; all six turns in the transcript (the 10 s grace read the rest of the conversation).
- Run on 2026-11-07 (S-17, TestCall): PASS. The transcript had one more Me line after the hang-up (#140).
- Original case checked the tray icon turning armed and recording and the menu items; with `echoctl status` as the stand-in for the icon, that part is now `recording/TC-019` (person).
- Tools the steps use outside the usual rules (`app_profile.md`, Recording with file-fed calls): `nohup`, `grep` on the run's own files.
