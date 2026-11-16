---
id: TC-032
title: A Google Meet call in Chrome is recorded like a Teams call — the arm starts on it, two tracks, meta.json says "meet"
priority: critical
type: functional
requirements: [F-REC-21, F-REC-07, F-REC-02]
module: recording
run: agent
platform: macos
data: small
tags: [recording, google-meet, chrome, watcher, tracks]
---

# TC-032: A Google Meet call in Chrome is recorded

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- As `recording/TC-031`, whose app and arm ("REC-031 next call") this case goes on with; on its own: TC-031 steps 1–2 first (the call then ended with `$C end`)
- The speech model linked into the run's config (the prelude of S-18, or `recording/TC-001` step 1)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Prelude; `$C start acme-sync --meet --title "Meet – tst-wxyz-abc - Google Chrome"`; `$W 20 recording rec-031-next-call`; `target/debug/echoctl status` | Within 20 s `watcher.in_call` true, `watcher.app` "meet", `watcher.recording` a folder ending `-rec-031-next-call`, `armed` null (the arm used up on this call) |
| 2 | Prelude; `$W 60 event recording_stopped`; `tail -n +$(( $(cat "$D/ev.start") + 1 )) "$ECHOCTL_HOME/.echo/events.jsonl" \| grep -e '"call_started"' -e '"call_ended"'` | The call hung up by itself (37 s); `call_started` and `call_ended` each carry `"app":"meet"` |
| 3 | Prelude; `CALL="$(ls -d "$ECHOCTL_HOME"/calls/*-rec-031-next-call)"`; read `"$CALL/meta.json"` | `app` "meet", `title` "REC-031 next call", `stop_reason` "call_ended"; `tracks` lists `mic.wav` and `remote.wav`, each with `frames` above 0, within 0.5 s of each other |
| 4 | Prelude; `$W 120 file "$CALL/transcript.md"`; `grep -c -e "Berlin first" -e "training room" "$CALL/transcript.md"` | 2 or more: transcribed from both tracks as any call |

## Expected Final State
One call folder, recorded from the Meet call in Chrome, with `app` "meet".

## Teardown
- `target/debug/echoctl app quit` (in S-18: left for `recording/TC-033`); `tests/fixtures/serve.sh down <dir>` on its own

## Notes
- Written from the feature's text (F-REC-21). That nothing is read from Teams during a Meet call (mute, names, shared screens, chat) can't show here: a file-fed call reads no window at all; it is `watch::run_loop`'s, by hand in `recording/TC-034`. The tap on Chrome's real audio (every tab's sound) and its real mic are the live check's (`recording/TC-034`, person): here both tracks come from files.
