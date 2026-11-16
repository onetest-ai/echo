---
id: TC-011
title: Transcription: Keep audio files on is saved to config.json
priority: medium
type: functional
requirements: [F-SET-11, F-SET-03]
module: settings
run: agent
platform: any
tags: [settings, audio]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-011: Transcription: Keep audio files on is saved to config.json

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<data>/.echo/config.json` key `keep_audio`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/transcription`; read the switch "Keep audio files" | It is off |
| 2 | Click it | It is on |
| 3 | Wait 1 s, run `. <dir>/env && grep -n '"keep_audio"' "$ECHOCTL_HOME/.echo/config.json"` | Prints `"keep_audio": true,` |

## Expected Final State
Keep audio files is on in the page and in `config.json`.

## Teardown
- Click the switch so it is off again.
