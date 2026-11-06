---
id: TC-010
title: Transcription lists the speech models with their download size and starts none
priority: high
type: functional
requirements: [F-SET-09]
module: settings
run: agent
platform: any
tags: [settings, transcription]
---

# TC-010: Transcription lists the speech models with their download size and starts none

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/transcription` | The heading "Transcription" and the card "Speech models" show |
| 2 | Read the row "large-v3-turbo-q5_0" | It has a button "Download (574 MB)" and its text ends "574 MB" |
| 3 | Read the rows of the card | "large-v3-turbo-q8_0" has "Download (874 MB)", "large-v3-q5_0" has "Download (1081 MB)", "small-q8_0" has "Download (264 MB)" |
| 4 | Look for the tag "Recommended for this computer" | Exactly one row carries it |
| 5 | Read the row "nemotron-3.5-0.6b" | It carries the tag "Not in this build" and has no download button |
| 6 | Look for a progress bar or the text "Downloading" | None shows |

## Expected Final State
The model list shows sizes and no download runs.

## Notes
- Never press a Download button here (about 0.3 to 1 GB from the network). Which row is Recommended depends on the computer.
