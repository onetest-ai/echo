---
id: TC-235
title: A moment opens in the right panel's What was said with the turns around it, and Open at opens the call at that moment
priority: high
type: functional
requirements: [F-CALLS-22]
module: calls
run: agent
platform: any
tags: [calls, search, recall, panel]
---

# TC-235: A moment opens in What was said and opens the call there

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-233: standard data served at `{{base_url}}`, Recall on and synced, the page loaded again

## Test Data

| Field | Value |
|-------|-------|
| Words | security review |
| The moment | Acme security review, Transcript, Sam Rivera, 0:35: "Agreed. The SSO checklist is ready in draft; I will send it once security signs off on the SAML settings." |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `security review`, press Enter | Results as in TC-233; "Acme security review" is open with three moments |
| 2 | Click the moment "Agreed. The SSO checklist is ready in draft…" | It is selected (violet); the right panel opens on a tab "What was said" (search icon, first tab) |
| 3 | Read the panel | A calls icon and "Acme security review"; "Yesterday, <time>", "40 min" and its people; chips "Acme" and "Acme rollout"; "TRANSCRIPT 0:35"; three turns: 0:20 Remote ("Sounds good…"), 0:35 Sam Rivera highlighted with "security" marked, 0:50 Remote ("Our security team…") |
| 4 | Read the row under the turns | A button "Open at 0:35", then icon buttons "Copy link" (tooltip "Copy the link to this moment (echo://)"), "Ask Claude" and "More"; below, "ALSO IN THIS CALL" with the call's two other moments |
| 5 | Click "More" | A menu with "Copy what was said" and "Search only Acme"; Escape closes it |
| 6 | Click a moment under "Also in this call" | The panel shows that moment (its time in "TRANSCRIPT m:ss") |
| 7 | Click "Open at …" | The call "Acme security review" opens on its Transcript tab with the turn of that time highlighted; the results stay in the list |

## Expected Final State
The moment was read in the right panel with its context, and its call opened at that moment.
