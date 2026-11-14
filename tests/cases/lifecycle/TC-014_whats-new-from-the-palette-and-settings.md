---
id: TC-014
title: What's new opens again from ⌘K and from Settings → General, beside Echo's version, with every version's items
priority: low
type: functional
requirements: [F-LIFE-12]
module: lifecycle
run: agent
platform: any
tags: [lifecycle, whats-new, palette, settings]
---

# TC-014: What's new from the palette and Settings

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/serve.sh up <dir> <port>` (nothing new to show: the running version is seen); the window is served at `{{base_url}}`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Load `{{base_url}}/`, wait for "Your day" and 3 s more | No "What's new" dialog |
| 2 | Press `ControlOrMeta+k`, type `What's new` | The group ECHO has "What's new" |
| 3 | Press Enter | The dialog "What's new in Echo" with a header per version, newest first (`api/whats-new`'s `entries`), each with its items |
| 4 | Click "Got it" | The dialog closes |
| 5 | Navigate to `{{base_url}}/#settings/general` | The row "Echo V" (the running version) has a button "What's new" beside "Check for updates" |
| 6 | Click "What's new" | The same dialog opens; press Escape: it closes, Settings stays |

## Expected Final State
What's new can be read again at any time from ⌘K and Settings; opening it changes nothing.
