---
name: start
description: Make sure Echo is running — the app that records the calls the user opts in to — and report its state. Use when the user says start/enable Echo, "record my meetings", or asks whether Echo is on. (The Echo mod starts the app by itself when a session opens.)
---

# echo: start

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`).

1. Run `echoctl up`. It starts the app if it is set up but not running, checks permissions
   (as the app sees them), syncs today's agenda from the calendar (the msgraph skill's CLI if
   installed — optional) and reports. Follow its `next`:
   - "run the setup skill…" → run the `setup` skill, then `echoctl up` again;
   - "the app is not running…" → tell the user, with the last lines of
     `<home>/.echo/app.log`;
   - "nothing" → running.
2. Say one line: running, and how many meetings are on today's agenda.

Everything else happens in the app: it refreshes the agenda every 15 min, asks "record?" with
its own dialog when a setting says `ask`, attaches the overlapping calendar meeting to a
recording and transcribes each call when it stops. The mod opens the live pane and runs
`process-call` when a transcript is ready (and, when a session opens, for calls transcribed
while none was open).

Without the mod (no fresh `<home>/.echo/mod.json` heartbeat), follow events yourself:
Monitor on `echoctl events --follow` (`timeout_ms: 1800000`, re-arm on expiry); on
`transcribed` with `ok: true`, `echoctl claim <dir>` and, if that succeeds, run `process-call`
on `dir`; on `error`, tell the user in one line (suggest `echoctl doctor` for permissions).

Ground rules: record only after an explicit yes for that call; never send anything without an
explicit yes for that item. The user records with the menu-bar icon, `/echo:record`
(optionally with a language) or "record", stops with `/echo:stop`, and adds notes with
`/echo:note`. `echoctl app quit` stops the app (a recording in progress is saved).
