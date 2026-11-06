---
id: TC-001
title: Every Settings section opens with its heading
priority: high
type: functional
requirements: [F-SET-01]
module: settings
run: agent
platform: any
tags: [settings, sections]
---

# TC-001: Every Settings section opens with its heading

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings` and wait for the heading "General" | The left list shows, in this order: General, Recording, Transcription, AI & minutes, Templates, Calendar & chat, Knowledge, About me, Sends to you, Plugins, Layout, Health; General is the open section and the text "Saved as you change it." is at the foot of the page |
| 2 | Click "Recording" in the left list | The page shows the heading "Recording" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 3 | Click "Transcription" in the left list | The page shows the heading "Transcription" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 4 | Click "AI & minutes" in the left list | The page shows the heading "AI & minutes" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 5 | Click "Templates" in the left list | The page shows the heading "Templates" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 6 | Click "Calendar & chat" in the left list | The page shows the heading "Calendar & chat" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 7 | Click "Knowledge" in the left list | The page shows the heading "Recall" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 8 | Click "About me" in the left list | The page shows the heading "About me" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 9 | Click "Sends to you" in the left list | The page shows the heading "Sends to you" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 10 | Click "Plugins" in the left list | The page shows the heading "Plugins" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 11 | Click "Layout" in the left list | The page shows the heading "Layout" (the first heading of the section); the text "This section couldn't be shown" is absent |
| 12 | Click "Health" in the left list | The page shows the heading "Health" (the first heading of the section); the text "This section couldn't be shown" is absent |

## Expected Final State
All twelve sections opened one after another, each with its own heading and no error text.

## Notes
- General and Health read the real machine (Claude Code, command line, permissions): this case checks the heading only, never their values, and keeps no screenshot of them.
- The Knowledge section's page is headed "Recall" (known naming issue in app_profile.md).
