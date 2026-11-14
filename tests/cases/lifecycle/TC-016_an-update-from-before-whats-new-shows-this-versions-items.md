---
id: TC-016
title: An update from a version before What's new existed (calls, nothing seen) shows only the running version's items, once
priority: medium
type: functional
requirements: [F-LIFE-10]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new, update]
---

# TC-016: An update from before What's new shows this version's items

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built with no What's new seen at all: `tests/fixtures/serve.sh up <dir> <port> --whats-new-seen none`; it holds calls (an Echo in use); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- In S-34 (stage glue): the page taken away (`about:blank`), then `python3 tests/fixtures/whats_new_seen.py none` after `. <dir>/env`. On its own: the `serve.sh up` flag above. Either way, `{{base_url}}` is not loaded before the step that loads it: its first load shows What's new and counts it as seen

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `ls "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"; ls "$ECHOCTL_HOME/calls" \| wc -l` | "No such file or directory"; at least one call |
| 2 | Load `{{base_url}}/`; in `browser_evaluate` read `api/whats-new` (`version`, `entries[0].version`) | Both are the running version V |
| 3 | Snapshot | A dialog "What's new in Echo V" with `entries[0]`'s items only: no older version's items, no version headers |
| 4 | Click "Got it"; run `cat "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | The dialog closes; `version` V |
| 5 | Reload `{{base_url}}/`, wait 3 s | No dialog |

## Expected Final State
The user who updated from a version without What's new saw this version's items once.
