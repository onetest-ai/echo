---
id: TC-029
title: Settings → General → Where drafts go says where mail drafts go now and saves the user's choice — the default mail app, another app, or Echo's default again
priority: medium
type: functional
requirements: [F-SET-20]
module: settings
run: agent
platform: any
data: standard
tags: [settings, mail, drafts]
---

# TC-029: Where drafts go

**Module:** Settings | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built and served right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (or the small set); the window at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Microsoft 365 is not signed in (the test data's default)
- The app's name `<APP>`: on macOS `Spark`; on Windows the full path of a program that exists (`target\debug\echoctl.exe` as its full path)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/general`; read the row "Where drafts go" | Its line: "Mail Claude writes for you — on its own (Yolo) or with echoctl compose — waits there as a draft; you send it. Now: the default mail app (the default)."; a select showing "Echo's default", with "Microsoft 365 Drafts", "The default mail app" and "Another app…" |
| 2 | Choose "The default mail app"; then `echoctl config` | The line ends "Now: the default mail app."; the config has `"drafts": "mail-app"` |
| 3 | Choose "Another app…" | A field "The mail app" shows beside the select (macOS placeholder "Its name: Microsoft Outlook", Windows "Its program: C:\…\OUTLOOK.EXE") |
| 4 | Type `<APP>` in it and leave the field (Tab); then `echoctl config` | The line ends "Now: <APP>." (on Windows: the full path); the config has `"drafts": "app:<APP>"` |
| 5 | Choose "Echo's default"; then `echoctl config` | The line is step 1's again; no `"drafts"` in the config |

## Expected Final State
Where drafts go is Echo's default.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Signed in to Microsoft 365, the default line says "Now: Microsoft 365 Drafts (the default when Microsoft 365 is connected)." (`drafts.test.mjs`).
