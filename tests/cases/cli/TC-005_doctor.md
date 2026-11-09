---
id: TC-005
title: echoctl doctor reports permissions from the running app, as Health does
priority: low
type: functional
requirements: [F-CLI-11, F-SET-17]
module: cli
run: agent
platform: macos
tags: [cli, doctor, second-instance]
external_id: TC-CLI-003
---

# TC-005: echoctl doctor reports permissions from the running app, as Health does

**Module:** CLI | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`); the window served on it at `{{base_url}}`
- **The dev app running on it (shared in S-19):** in S-19, `claude/TC-015`'s (started by its step 2, its teardown skipped). On its own: `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"` after `. <dir>/env`, then `target/debug/echoctl app quit` at the end
- The speech model linked into the test config (`ln -s "$HOME/Library/Application Support/echo/models" "$ECHOCTL_CONFIG_DIR/models"`, `target/debug/echoctl model use large-v3-turbo-q5_0`; in S-19 the prelude's)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `. <dir>/env; target/debug/echoctl doctor --json` | `checked_by` is "app" (the running app answered); four checks: `microphone`, `system-audio`, `accessibility`, `screen`, each with a `status` (granted / denied / unknown), a `how` and a `settings_url`; `ready` true when the two required ones are granted |
| 2 | `{{base_url}}/#settings/health` | PERMISSIONS lists Microphone, System Audio Recording, Accessibility and Screen Recording with the same states as step 1 (✓ for granted) |
| 3 | `. <dir>/env; target/debug/echoctl up` | `model` is "large-v3-turbo-q5_0" (the speech model the app uses), `app_pid` the running app's, `ready` true |

## Expected Final State
doctor answers from the running app, matching Health; `up` names the speech model.

## Notes
- `doctor --json` also has `ai` (#224): with `--ai fake` it is empty (small data has no local AI profile); with `--ai local`, doctor first loads the data's model to read its context, which can take a few seconds.
- The permissions are the terminal's that started the dev app (`responsible_app`, e.g. the terminal's bundle id): on a real install they are Echo's own (S-15/S-16).
- The tray's Health submenu is not compared: the run's terminal can't tell the dev app's menu-bar item from the installed Echo's through System Events (`recording/TC-019`).
- Run on 2026-11-07 (S-17's app): PASS — `checked_by: app`, all four granted, Health the same; `up` names the model.
