---
name: record
description: "Use when the user says \"record this call\", \"record in Ukrainian\", \"record a memo\", \"let me dictate a note\", \"stop recording\" or \"don't record this\". Starts or stops Echo's recording of a Teams call, or a memo — the user alone, no call — optionally pinning its language and title."
argument-hint: "[stop] | [memo] | [language: en|uk|ru|es…] [title]"
---

# echo: record / stop

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Record only when the user asked
for this call.

**Stop** ("stop", "don't record this"): `echoctl stop` and report its one-line output. The app
transcribes the call and the notes follow by themselves.

**Record:**
1. `echoctl status`. If `watcher` is null the app isn't running: `echoctl app start` (if that
   fails, run the `doctor` skill).
2. `echoctl arm`, with `--title "<title>"` if the user gave one and `--languages <code>` if they
   named the language ("in Ukrainian" → `--languages uk`); without one the call is in the default
   language. A call has one language; "in English and Ukrainian" → ask which. The app attaches the
   calendar meeting by itself.
3. One line from the output: a call in progress → "● Recording"; no call yet → "Armed —
   recording starts when your next Teams call begins (15 min)".

**Memo** ("record a memo", "let me dictate"): the user alone, no Teams call. `echoctl memo start` (`--languages
uk` when they named one); it starts at once. `echoctl memo stop` (or `stop`) ends it. Its threads are found from what
was said and its minutes come right after: nothing to choose. Refused while something records — say so.
