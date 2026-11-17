---
id: TC-014
title: "AI & minutes: Claude's permissions shows three settings side by side — Claude in the window, routines and Yolo acting on their own — with their flags and warning, each on Auto by default"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
tags: [settings, claude, permissions]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-014: AI & minutes: Claude's permissions shows its three settings, each on Auto by default

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` (the small set: none of the three settings written, so each is on its default), window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; find the card "Claude's permissions" | Three rows, in this order: "Claude in the window", "Routines acting on their own", "Yolo acting on its own" |
| 2 | Read the row "Claude in the window" | It names `--permission-mode auto` and `--dangerously-skip-permissions`, says Claude "runs any command and tool without asking" and "Takes effect the next time the session starts."; its buttons "Ask each time", "Auto" (pressed: the default) and "Skip all permission checks"; no "Skips every check." |
| 3 | Read the rows "Routines acting on their own" and "Yolo acting on its own" | Each names `--permission-mode auto` and `--dangerously-skip-permissions`; its buttons "Auto" (pressed: the default) and "Skip all permission checks", no "Ask each time": a routine and Yolo always act on their own |
| 4 | Read the card's last line | "Auto on your profiles:" with each of the data's profiles and what Auto gets there — on a Claude account or Claude Code "auto mode: Claude Code decides each action" |
| 5 | Run `. <dir>/env && grep -c -E '"(folder_skip|folder|routine|yolo)_permissions"' "$ECHOCTL_HOME/.echo/config.json"` | `0`: none written, each on its default |

## Expected Final State
The three settings show their defaults; nothing changed.

## Notes
- No button is pressed here: changing them, the CLI and the old switch's move are `settings/TC-095`.
