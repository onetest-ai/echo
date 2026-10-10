---
name: record
description: Record the current Teams call now, or the next call the user joins. Use when the user says "record", "record this call", "start recording", or wants an unscheduled/off-calendar call captured.
argument-hint: "[language: en|uk|ru|es…] [title]"
---

# echo: record this call

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`).

1. `echoctl status`. If `watcher` is null the app isn't running: `echoctl app start` (if that
   fails, run the `setup` skill), then continue.
2. Arm: `echoctl arm` — add `--title "<title>"` if the user gave one, and
   `--languages <codes>` if they named the call's language(s) ("record in Ukrainian", "uk",
   "en+uk" → `--languages uk` / `--languages en,uk`). The watcher attaches the overlapping
   calendar meeting by itself. (The same as Record in the menu-bar icon.)
3. Report in one line, from the command's output:
   - call in progress → "● Recording" (the `recording_started` event follows within a second);
   - no call yet → "Armed — recording starts when your next Teams call begins (15 min)".
