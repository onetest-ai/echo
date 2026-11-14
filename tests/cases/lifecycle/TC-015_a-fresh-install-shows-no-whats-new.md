---
id: TC-015
title: With no calls and no What's new seen (a fresh install), the window shows no What's new
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
- In S-34 (stage glue): the page taken away (`about:blank`), then `python3 tests/fixtures/whats_new_seen.py none` after `. <dir>/env`. On its own: the `serve.sh up` flag above. Either way, `{{base_url}}` is not loaded before the step that loads it: its first load shows What's new and counts it as seen

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `ls "$ECHOCTL_HOME/calls"; ls "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | No calls; "No such file or directory" |
| 2 | Load `{{base_url}}/`, wait for "Your day" and 3 s more; snapshot | No "What's new" dialog |
| 3 | Run `ls "$ECHOCTL_CONFIG_DIR/whats-new-seen.json"` | "No such file or directory": the window's asking wrote nothing |
| 4 | Reload `{{base_url}}/`, wait 3 s | Still no dialog |

## Expected Final State
Nothing was shown with nothing to compare with; nothing was written.

## Notes
- A real fresh install also starts with its own version seen: the app's first run (`app::first_run`, the data folder question) writes it — a run of the installed app, `install/` cases and Rust; not served data. With calls and none seen, Echo was in use before What's new existed: `TC-016`.
