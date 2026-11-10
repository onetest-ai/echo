---
id: TC-025
title: An unreadable config.json says so in every Settings section instead of its settings; the rest of the window works
priority: medium
type: functional
requirements: [F-SET-18]
module: settings
run: agent
platform: any
data: standard
tags: [settings, files]
auto: [tests/auto/specs/S-29.spec.mjs]
---

# TC-025: An unreadable config.json says so in every Settings section instead of its settings; the rest of the window works

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); shell steps run after `. <dir>/env`
- **This case breaks and restores one file in the run's data folder** (`$ECHOCTL_HOME/.echo/config.json`): the one exception to the runner's read-only Bash, on the run's own data only. Always run the teardown

## Test Data

| Field | Value |
|---|---|
| Broken content | `{ "languages": [` (JSON cut short) |
| Error | `EOF while parsing a list at line 1 column 16` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `cp "$ECHOCTL_HOME/.echo/config.json" "$(dirname "$ECHOCTL_HOME")/config.json.bak"` then `printf '{ "languages": [' > "$ECHOCTL_HOME/.echo/config.json"` | The copy exists; the file holds the broken text |
| 2 | Run `target/debug/echoctl config` | Exit code 1; "Error: EOF while parsing a list at line 1 column 16" |
| 3 | Reload `{{base_url}}/#settings/general` | The Settings list on the left is there; the page reads "Echo's settings (config.json) can't be read: EOF while parsing a list at line 1 column 16" and no setting (no Appearance, no switches) |
| 4 | Click the sections Recording, AI & minutes, About me and Plugins (`[data-setsec="recording"\|"ai"\|"me"\|"plugins"]`) | Each shows the same sentence and no settings |
| 5 | Click "Calls" in the activity bar | The calls list is shown as usual and, since no call was opened in this page load (step 3 reloaded on Settings), the pane reads "Pick a call"; no error. Click the first row (the newest call, Globex standup): it opens on its Minutes tab, whose box says "Automatic minutes can't start" and "Echo's settings can't be read: EOF while parsing a list at line 1 column 16" (its automatic minutes need the settings) and still offers "Write minutes"; the window shows no other error |
| 6 | Run `cp "$(dirname "$ECHOCTL_HOME")/config.json.bak" "$ECHOCTL_HOME/.echo/config.json"`, reload `{{base_url}}/#settings/general` | The General section shows its settings again (Appearance System/Light/Dark); `target/debug/echoctl config` exits 0 |

## Expected Final State
`config.json` is as the data built it; Settings shows its sections.

## Teardown
- If the case stopped before step 6, run step 6's `cp`

## Notes
- The page still shows its foot line "Saved as you change it." under the error, though nothing can be changed then. Not asserted.
