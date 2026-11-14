---
id: TC-011
title: After an update, the window shows What's new for the new version once, and a reload doesn't show it again
priority: medium
type: functional
requirements: [F-LIFE-10, F-LIFE-11]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new, update]
---

# TC-011: After an update, What's new shows once

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built as an update from the version before this one: `tests/fixtures/serve.sh up <dir> <port> --whats-new-seen previous` (the last version seen is the entry before this build's in What's new); the window is served at `{{base_url}}`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- In S-34 (stage glue): the page taken away (`about:blank`), then `python3 tests/fixtures/whats_new_seen.py previous` after `. <dir>/env`. On its own: the `serve.sh up` flag above. Either way, `{{base_url}}` is not loaded before the step that loads it: its first load shows What's new and counts it as seen
- The dialog is `[role="dialog"]` named "What's new…"; its items are `.wn-item`
- The newest entry is the running version's (a release can't ship without it): read both from the page in step 2

## Test Data

| Field | Value |
|---|---|
| Last version seen | the one before the running version (`whats-new-seen.json` in the run's config folder) |
| Running version V | `target/debug/echoctl --version` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl --version` and `cat "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | "echoctl V"; the file holds a `version` older than V |
| 2 | Load `{{base_url}}/`; in `browser_evaluate` run `async () => (await (await fetch('api/whats-new')).json())` | JSON `{version: V, entries: [...]}`, `entries[0].version` is V |
| 3 | Snapshot the page | A dialog "What's new in Echo V" over the window: one item per item of `entries[0]`, in its order, each with an icon, its title in bold and its one sentence; a "Show me" button on each item that has `open`, none on one that hasn't; a "Close" (×) button and "Got it". No other version's items, no version headers |
| 4 | Run `cat "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | `version` is now V: shown, it counts as seen |
| 5 | Click "Got it" | The dialog is gone; the window (Today) is as before |
| 6 | Reload `{{base_url}}/`, wait for "Your day" and 3 s more; snapshot | No "What's new" dialog |
| 7 | Navigate to `{{base_url}}/#calendar`, reload, wait 3 s | No "What's new" dialog |

## Expected Final State
What's new showed once for V; the config folder remembers V as seen; later loads of the window show nothing.

## Notes
- The items are the compiled-in `crates/echoctl/whats-new.json`; the case reads them from `api/whats-new`, so it holds for every release.
- Test data is otherwise built with the running version as seen (`--whats-new-seen` defaults to it), so no other case meets the dialog.
