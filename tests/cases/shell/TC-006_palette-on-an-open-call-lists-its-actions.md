---
id: TC-006
title: ⌘K on an open call lists that call's actions first
priority: medium
type: functional
requirements: [F-WS-03]
module: shell
run: agent
platform: any
tags: [shell, palette]
---

# TC-006: ⌘K on an open call lists that call's actions first

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, click the row "Acme weekly sync" (`[data-call$="acme-weekly-sync"]`) | The call opens on its minutes |
| 2 | Press `ControlOrMeta+k` | A dialog "Actions" opens with the box "Action" focused and a chip "On: Acme weekly sync ×" |
| 3 | Read the group names | THIS CALL, ECHO, GO TO and THREADS (and CLAUDE) |
| 4 | Read the group THIS CALL | "Open Minutes", "Open Transcript", "Write minutes again", "Ask Claude about this call", "Add a note", "Show the call folder", "Open minutes in browser", "Delete this call…" |
| 5 | Read the group THREADS | "Acme rollout" and "Globex pilot" with the hint "Active" |
| 6 | Press Escape | The dialog closes; the call is still open |

## Expected Final State
The palette listed the call's own actions and nothing was run.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
- None of the items is run: "Write minutes again" and "Ask Claude…" start Claude Code.
