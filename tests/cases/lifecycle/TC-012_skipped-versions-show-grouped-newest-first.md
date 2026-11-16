---
id: TC-012
title: After skipping versions, What's new lists each version's items since the last one seen, newest first under version headers; it reads at phone width and in dark mode, and × and Esc close it
priority: medium
type: functional
requirements: [F-LIFE-10, F-LIFE-11]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new, update, narrow, dark]
---

# TC-012: Skipped versions show grouped, newest first

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built as an update from a very old version: `tests/fixtures/serve.sh up <dir> <port> --whats-new-seen 0.0.1`; the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- In S-34 (stage glue): the page taken away (`about:blank`), then `python3 tests/fixtures/whats_new_seen.py 0.0.1` after `. <dir>/env`. On its own: the `serve.sh up` flag above. Either way, `{{base_url}}` is not loaded before the step that loads it: its first load shows What's new and counts it as seen
- Playwright viewport 1440×900 to start; the dialog is `[role="dialog"]` headed "What's new…" (its `h3`), its version headers `h4.wn-ver`, its items `.wn-item`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Load `{{base_url}}/`; in `browser_evaluate` run `async () => (await (await fetch('api/whats-new')).json()).entries.map((e) => [e.version, e.items.length])` | Every version with an entry, newest first (at least two) |
| 2 | Snapshot the dialog | Heading "What's new in Echo"; a header "Echo <version>" per version of step 1, in the same order (newest first), each followed by that version's items (the counts of step 1); "Show me" only on the items that have `open` (`api/whats-new`), none on one without |
| 3 | Press Escape | The dialog closes |
| 4 | Reload the page (`location.reload()`), wait 3 s | No dialog: shown once, every version up to the running one is seen |
| 5 | Press `ControlOrMeta+k`, type `What's new`, press Enter; resize the viewport to 390×844; screenshot | The dialog fits the width (no sideways scroll: `document.scrollingElement.scrollWidth <= 390`); the items read in one column, each title and sentence whole (none cut off), and the list scrolls inside the dialog while its heading and "Got it" stay |
| 6 | `browser_emulate_media` `colorScheme: 'dark'` (or Settings → General → Appearance → Dark); screenshot | The dialog in the dark theme: dark surface, light text, the icons visible |
| 7 | Click "Close" (×) | The dialog closes |

## Expected Final State
What's new listed every version since the last one seen, newest first; it closed with Esc and with ×; it reads at phone width and in the dark theme.

## Teardown
- Resize back to 1440×900; `browser_emulate_media` `colorScheme: 'light'` (or Appearance back to System)
