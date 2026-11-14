---
id: TC-015
title: A fresh install (no calls yet) shows no What's new, and its version counts as seen
priority: medium
type: functional
requirements: [F-LIFE-10]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new, first-run]
---

# TC-015: A fresh install shows no What's new

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built with no What's new seen at all: `tests/fixtures/serve.sh up <dir> <port> --whats-new-seen none` (no `whats-new-seen.json` in the run's config folder); the window is served at `{{base_url}}` but **not loaded yet**; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- A fresh install has no calls: empty the data folder's calls with the CLI before the window is loaded:
  `for c in $(ls "$ECHOCTL_HOME/calls"); do target/debug/echoctl delete "$c"; done` (they go to the trash)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `ls "$ECHOCTL_HOME/calls"; ls "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | No calls; "No such file or directory" |
| 2 | Load `{{base_url}}/`, wait for "Your day" and 3 s more; snapshot | No "What's new" dialog |
| 3 | Run `target/debug/echoctl --version; cat "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | "echoctl V"; the file holds `version` V: a fresh install starts with its own version seen |
| 4 | Reload `{{base_url}}/`, wait 3 s | Still no dialog |

## Expected Final State
Nothing was shown on the fresh install; the next update will show only what's newer than V.

## Notes
- Fresh means no What's new seen and no calls. With calls and none seen, Echo was in use before What's new existed: `TC-016`.
