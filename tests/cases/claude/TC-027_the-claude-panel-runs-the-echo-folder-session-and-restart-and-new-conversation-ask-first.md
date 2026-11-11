---
id: TC-027
title: The Claude panel (status bar, ⌘`, ⌘K) runs the Echo-folder session, closing the window keeps it, and Restart and New conversation ask first while it runs
priority: medium
type: functional
requirements: [F-CC-05]
module: claude
run: person
platform: macos
data: small
tags: [claude, panel, session, window]
---

# TC-027: The Claude panel runs the Echo-folder session; closing the window keeps it

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in on this Mac (your own account; the session talks to it)
- **Made-up data, never `~/echo`.** In the checkout, after `cargo build`:
  1. `D="$(mktemp -d)/echo-test"; python3 tests/fixtures/make_data.py "$D" --ai fake` (`--ai fake` turns the app's thread sync off)
  2. `. "$D/env"; export PATH="${PATH#"$D/bin:"}"; unset CLAUDE_CONFIG_DIR ECHOCTL_NOTES_NO_SHELL` — the app and its Claude panel then start your real Claude Code (the data's fake stays out of PATH)
  3. `target/debug/echoctl ai default system`, `target/debug/echoctl config --scheduled manual --unscheduled manual --auto-minutes false`, `target/debug/echoctl ai live off`
  4. `nohup target/debug/echoctl app run > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"` (a second app on the test data; its lock is in the test config folder, so your own Echo is not refused). The app may run a few small Claude Code jobs by itself on your account about a minute after it starts (day preps, meeting types): they only read the made-up data
- Your own Echo may be running: the window under test is the one `target/debug/echoctl app open` opens (it talks to the app of this data folder only); if two Echo icons show in the menu bar, use only the window that opened
- The window shows the made-up data (Acme weekly sync, Initech planning in Calls)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `. "$D/env"; target/debug/echoctl app open` (with step 2's PATH undo applied in this shell too); in the window look at the status bar at the bottom | A button "Claude" in the status bar (its tooltip "Claude (⌘`)") |
| 2 | Click "Claude" (or press ⌘`) | The bottom panel opens on its tab "Claude" with the text "Ask Claude about anything in Echo — the call or thread you have open (it sees what you are looking at), your clients, people, tasks. It is your own Claude Code, with your settings, working in Echo's data folder." and a button "Start Claude" |
| 3 | Click "Start Claude" | "Starting Claude Code…", then Claude Code's own prompt in the panel; the status bar button reads "● Claude" |
| 4 | In the session ask `What folder are you working in? Answer with the path only.` | The answer is the made-up data folder (`$D/home`), not `~/echo` |
| 5 | Press ⌘` | The panel hides and the session keeps running (the button is still "● Claude"); ⌘` again shows it with the same conversation |
| 6 | Open the call Acme weekly sync in the window, then in the session ask `Which call am I looking at in Echo? Answer with its title only.` | The answer names "Acme weekly sync" (the session knows what the window shows) |
| 7 | Press ⌘K, type `Restart Claude`, press Enter | The panel shows an alert: "Restart Claude? Whatever it is doing now stops, and an answer being written is lost. The conversation is kept and goes on after the restart." with the buttons "Keep it running" and "Restart"; nothing has restarted yet |
| 8 | Click "Keep it running" | The alert goes; the session is the same one (the earlier answers are still on screen) |
| 9 | Press ⌘K, `Restart Claude`, Enter, then click "Restart" | "Starting Claude Code…", then the same conversation goes on: the question of step 6 and its answer are shown again |
| 10 | Press ⌘K, type `New conversation in Claude`, press Enter | The alert "Start a new conversation? This session stops, and whatever it is doing now is lost." with "Keep it running" and "Start new" |
| 11 | Click "Start new" | A fresh Claude Code conversation: the earlier questions are not shown |
| 12 | Ask `Say the word ready.` and wait for the answer; close the window (the red close button of Echo's window) | The window closes; `target/debug/echoctl app` still prints a `running` pid |
| 13 | `target/debug/echoctl app open`, then open the Claude panel (⌘`) | The same session is there, "● Claude" in the status bar, with its answer "ready" still in the panel: closing the window did not end it |

## Expected Final State
One Echo-folder session lives in the app: it survives hiding the panel and closing the window, goes on after a confirmed Restart, starts fresh after a confirmed New conversation, and neither asks nor acts before the confirmation.

## Teardown
- `. "$D/env"; target/debug/echoctl app quit` (the session ends with the app; if the app doesn't stop within 30 s, `kill $(cat "$D/app.pid")`), then `rm -rf "$(dirname "$D")"`.
- Nothing of this case reads or writes `~/echo`.

## Notes
- Person-run: it needs the native window (the traffic-light close, ⌘`, ⌘K) and a real, signed-in Claude Code; the served window can't keep a session beyond a page load.
- Steps 4 and 6 depend on the model answering; a wrong answer with `<echo_view>` missing from the prompt is the failure (the page reports the view into `<config dir>/window-view.json`; `view-hook` adds it to each prompt).
