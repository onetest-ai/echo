---
id: TC-028
title: A meeting from 3 minutes before its start to 4 minutes after shows in the pill with its time and "Record it", which arms it; outside that the pill is the idle one
priority: high
type: functional
requirements: [F-REC-04]
module: recording
run: agent
platform: macos
tags: [recording, pill, arm, agenda, second-instance]
---

# TC-028: A meeting about to start shows in the pill with its time and "Record it"

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `serve.sh up <dir> <port> --ai fake`): the small set, hermetic — the app this case starts runs Claude Code on its own and must reach no real account; `target/debug/echoctl` is the debug build from this checkout (`cargo build`). Build it between 06:00 and 21:00 local time: the meeting used (Globex design review) starts 1 h after the build and later in the evening the set squeezes it
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; export D`
- **Needs a second app instance** (the arm's state, "Armed", is the watcher's): as in `recording/TC-003`, started in step 1; if `$D/app.out` says "already running" the case is BLOCKED, not failed
- The window is served on the same data at `{{base_url}}` with clicks reaching the app (`serve.sh up`); the page is loaded from `{{base_url}}/` first
- **A clock for the page only.** The pill decides from `Date.now()` (the meeting's start − 3 min ≤ now ≤ start + 4 min, no call on, the meeting has a Teams link). Steps 3–7 set it in the page with `browser_evaluate`: `() => { Date.now = () => <ms> }`, where `<ms>` is the start of Globex design review (step 2) plus the offset the step names, in milliseconds. The override lasts until the page is reloaded; **never use `page.clock`** (it stays on the browser's context for the runs after this one). After an override, the page draws the pill again when a view is opened: click `nav[aria-label="Activities"] [data-go="calls"]`, then `[data-go="today"]`
- No call is on (`python3 tests/fixtures/call.py status`: `in_call` false), nothing armed or recording

## Test Data

| Field | Value |
|-------|-------|
| Meeting | Globex design review (`ev-globex-review`, Kim Park, Teams link `https://teams.example/ev-globex-review`), about 1 h after the build |
| Offsets from its start | −4 min (outside), −2 min, +3 min (inside), +5 min (outside) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl config --unscheduled manual --scheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `target/debug/echoctl status` | The config calls exit 0; "waited … for app"; `status` prints `watcher` with a `pid`, `recording` null, `armed` null; no "already running" in `$D/app.out` |
| 2 | Load `{{base_url}}/`, wait for "Your day"; in `browser_evaluate` run `async () => (await (await fetch('api/agenda')).json()).find((m) => m.id === 'ev-globex-review').start` | An ISO time with an offset, about an hour after now: this is START. Without the override the top bar's pill is the idle one: the button "Record next call" (`[data-cmd="record"]`) and "Memo"; no group "Meeting soon" |
| 3 | Override the page's clock to START − 4 min, redraw (the Calls button, then the Today button), snapshot the top bar | The idle pill: "Record next call" and "Memo"; no group "Meeting soon" (more than 3 minutes before the start) |
| 4 | Override the clock to START − 2 min, redraw, snapshot the top bar | A group "Meeting soon" (`.pill.soon`) with the meeting's start time as the browser writes it (`new Date(START).toLocaleTimeString([], {hour: '2-digit', minute: '2-digit'})`, e.g. "02:30 PM"), the text "Globex design review" and a button "Record it" (`[data-arm="ev-globex-review"]`); no "Record next call" and no "Memo" button |
| 5 | Override the clock to START + 3 min, redraw, snapshot | The group "Meeting soon" is still there, with the same time, title and "Record it" |
| 6 | Override the clock to START + 5 min, redraw, snapshot | The idle pill again: "Record next call" and "Memo"; no group "Meeting soon" (more than 4 minutes after the start) |
| 7 | Override the clock to START − 2 min, redraw, click "Record it"; wait up to 5 s; then run the prelude and `target/debug/echoctl status` | The pill is a group "Armed" reading "Armed — records the next Teams call" with a button "Cancel"; no "Meeting soon"; `status` prints `armed` with `"title": "Globex design review"` and `watcher.armed` true |
| 8 | Click "Cancel"; wait up to 5 s; run the prelude and `target/debug/echoctl status` | `armed` is null; the pill shows "Meeting soon" again (the meeting is still close) with "Record it" |

## Expected Final State
The pill offered the meeting only from 3 minutes before to 4 minutes after its start; "Record it" armed that meeting by its title and "Cancel" disarmed it. Nothing was recorded; `status` shows `armed` null.

## Teardown
- Reload `{{base_url}}/` (drops the page's clock override)
- Run the prelude, then `target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`
- `tests/fixtures/serve.sh down <dir>` and remove `$(dirname <dir>)` when the run is over

## Notes
- The rule is `meetingSoon` in `10-shell.js`: a meeting with an id and a Teams link (not a Zoom or Google Meet one), 3 minutes before its start to 4 after, while no call is on (`st.in_call` false). A call already on shows "Record this call" instead of the meeting: not checked here.
- "Record it" is `arm_meeting` (the meeting's subject and context are armed); the recording that follows when the call begins, attached to the meeting, is `recording/TC-001` and `TC-012`.
- The real clock is not touched, so the case runs at any hour the data can be built; only the page's `Date.now` moves.
