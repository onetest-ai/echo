---
id: TC-016
title: Open on an item goes to the place the item is about, and Change… goes to the People tab
priority: medium
type: functional
requirements: [F-INBOX-04, F-INBOX-06]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, navigation]
---

# TC-016: Open on an item goes to the place the item is about, and Change… goes to the People tab

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Wait 10 seconds after the server starts: the sign-in check of the AI profiles settles in the background, and only then the blocking item "Sign in to Claude · out" is in the Inbox (the very first answer lists 24 open items and none blocking)

## Test Data

| Item | Button | Lands on |
|---|---|---|
| Minutes failed: Acme security review | Open | `#call/<folder>/minutes` |
| 3 to review in Acme rollout's work | Open → | `#project/acme-rollout/work` |
| Is Rita Gomez a champion? | Change… | `#project/acme-rollout/people` |
| Teams transcript: Acme design review | Open | `#calendar/day` |
| Sign in to Claude · out | Open | `#settings/ai` |
| Crm Sync 1.1.0 is available | Open → | `#settings/plugins/crm-sync` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Click "Open" on "Minutes failed: Acme security review" | The URL ends `#call/<folder>/minutes` with the folder `…-acme-security-review`; the panel stays open |
| 3 | Click "Open →" on "3 to review in Acme rollout's work" | The URL is `#project/acme-rollout/work` |
| 4 | Click "Change…" on "Is Rita Gomez a champion?" | The URL is `#project/acme-rollout/people` |
| 5 | Click "Open" on "Teams transcript: Acme design review" | The URL is `#calendar/day` |
| 6 | Click "Open" on "Sign in to Claude · out" | The URL is `#settings/ai` |
| 7 | Click "Open →" on "Crm Sync 1.1.0 is available" | The URL is `#settings/plugins/crm-sync` |

## Expected Final State
The window is on the plugin's page.

## Teardown
- Navigate to `{{base_url}}/#today` and click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
- Only the URL is asserted for Settings pages: they read the real machine (app_profile.md, Fragile areas).
