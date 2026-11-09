---
id: TC-024
title: While a call records, the window draws only what changed — new transcript turns, the clock's text — and the recording dot pulses without redrawing the window
priority: high
type: functional
requirements: [F-LIVE-08]
module: window
run: agent
platform: macos
tags: [window, performance, live-transcript, file-fed]
---

# TC-024: A recording window draws only what changed

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- The same setup as `recording/TC-015`: test data from `tests/fixtures/make_data.py <dir> --ai fake`, calls fed from files, the debug build, the speech model linked, a second app instance on the test data. The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`
- Live notes on (`target/debug/echoctl ai live on`) on any AI profile (nothing here needs a summary), languages `en`, `--unscheduled manual --scheduled manual --auto-minutes false`
- The browser tab is visible (`document.visibilityState` is "visible"): a hidden page doesn't refresh

## Test Data

| Field | Value |
|-------|-------|
| Conversation | minutes-call (213 s), held |
| Arm title | WIN-024 |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app; target/debug/echoctl arm --title WIN-024; $C start minutes-call --hold; $W 20 recording win-024`, then `ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve 48095 > "$D/serve.log" 2>&1 & echo $! > "$D/serve.pid"` | "waited … for recording win-024"; the serve log prints the address |
| 2 | Load `http://127.0.0.1:48095/`, then navigate to `#call/<the call's folder name>/transcript` (`basename $(ls -d $ECHOCTL_HOME/calls/*-win-024)`) | The live transcript, the pill with a running clock |
| 3 | Evaluate in the page: note the clock's text, the number of `.transcript[data-turns] > .turn` and the first turn element; wrap `window.patch` to count the areas whose HTML changed; wait 20 s; read the same again | The clock moved on by about 20 s; there are more turns than before; **the first turn is the same element** (not drawn again); **no area was patched** (the count is empty) |
| 4 | Evaluate `getComputedStyle(document.querySelector('.pill.rec .dot')).animationName` and the `rec-pulse` keyframes' property (`[...document.styleSheets].flatMap((s) => [...s.cssRules]).find((r) => r.name === 'rec-pulse').cssText`) | `rec-pulse`, animating `opacity` only (no `box-shadow`) |
| 5 | Run the prelude, then `$C end; $W 60 not-recording; $W 120 event transcribed`; navigate to the call's `#…/transcript` again; evaluate `S.find = 'berlin'; render()` and count `.transcript .turn.hit`, then `S.find = ''; render()`, then `S.jump = 20; render()` and look for `.transcript #jump` | 1 or more hits, then none; the `#jump` turn is there |

## Expected Final State
The live transcript grows without the window being redrawn; the final transcript's search and jump still work.

## Teardown
- Navigate the tab to `about:blank`. Run the prelude, then `kill $(cat "$D/serve.pid"); target/debug/echoctl app quit`; if it does not stop within 30 s, `kill $(cat "$D/app.pid")`

## Notes
- Measured 2026-11-09 (M4 Max, the app's own window open on this recording): the window's WebKit processes 17.7% CPU on main, 0.8% with the change; the app 4.7% → 1.6%. The pulse animated `box-shadow`, which repaints every frame, and the top bar was rebuilt every 2 s for its clock text.
- A window nobody sees (minimised, covered) fetches and draws nothing until it is shown again: not checkable from a served tab.
