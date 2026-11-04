---
name: record
description: "Use when the user says \"record this call\", \"record in Ukrainian\", \"stop recording\" or \"don't record this\". Starts or stops Echo's recording of a Teams call, optionally pinning its language and title."
argument-hint: "[stop] | [language: en|uk|ru|es…] [title]"
---

# echo: record / stop

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Record only when the user asked
for this call.

**Stop** ("stop", "don't record this"): `echoctl stop` and report its one-line output. The app
transcribes the call and the notes follow by themselves.

**Record:**
1. `echoctl status`. If `watcher` is null the app isn't running: `echoctl app start` (if that
   fails, run the `doctor` skill).
2. `echoctl arm`, with `--title "<title>"` if the user gave one and `--languages <codes>` if they
   named the language ("in Ukrainian" → `--languages uk`, "en+uk" → `--languages en,uk`). The
   app attaches the calendar meeting by itself.
3. One line from the output: a call in progress → "● Recording"; no call yet → "Armed —
   recording starts when your next Teams call begins (15 min)".
