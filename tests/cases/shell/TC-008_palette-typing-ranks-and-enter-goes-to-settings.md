---
id: TC-008
title: Typing in the palette ranks the matches and Enter runs the first
priority: medium
type: functional
requirements: [F-WS-03]
module: shell
run: agent
platform: any
tags: [shell, palette]
---

# TC-008: Typing in the palette ranks the matches and Enter runs the first

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`, press `ControlOrMeta+k` and type `plugins` | The first item is "Settings: Plugins" in the group GO TO and it is highlighted |
| 2 | Press Enter | The dialog closes; the URL hash is `#settings/plugins`; the heading "Plugins" shows |
| 3 | Press `ControlOrMeta+k`, type `globex pilot` | The item "Globex pilot" shows in the group THREADS and a call row "Globex pilot kickoff" in the group CALLS |
| 4 | Press Escape | The dialog closes |

## Expected Final State
The palette went to Settings: Plugins from a few typed letters.

## Notes
- Keys are written with Playwright's `ControlOrMeta` (⌘ on macOS, Ctrl elsewhere); the window follows the browser's platform.
