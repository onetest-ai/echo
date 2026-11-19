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
| The moment | Acme security review, Transcript, Remote, 0:50: "Our security team wants to review the questionnaire answers before anything goes live in production." (the turn of its section with both words) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `security review`, press Enter; find the source row "Acme security review" ("1 moment"), and click it once if it is closed (any row but the first when there are more than two sources) | Results as in TC-233; the row is open and its one moment shows under it |
| 2 | Click the moment "Our security team wants to review the questionnaire answers…" | It is selected (violet); the right panel opens on a tab "What was said" (search icon, first tab) |
| 3 | Read the panel | A calls icon and "Acme security review"; "Yesterday, <time>", "40 min" and its people; chips "Acme" and "Acme rollout"; "TRANSCRIPT 0:50"; three turns: 0:35 Sam Rivera ("Agreed. The SSO checklist…"), 0:50 Remote highlighted with "security" and "review" marked, 1:05 Sam Rivera ("Then we keep the pilot…") |
| 4 | Read the row under the turns | A button "Open at 0:50", then icon buttons "Copy link" (tooltip "Copy the link to this moment (echo://)"), "Ask Claude" and "More"; no "ALSO IN THIS CALL" (the call has no other moment with the words) |
| 5 | Click "More" | A menu with "Copy what was said" and "Search only Acme"; Escape closes it |
| 6 | Click "N more sources" if the source row "Acme weekly sync" that has "2 moments" isn't listed; click that row (it is closed) and then its first moment | The panel shows it, and under it "ALSO IN THIS CALL" with the other moment; clicking that one shows it in the panel (its time in "TRANSCRIPT m:ss") |
| 7 | Click "Open at …" | The call of the moment shown ("Acme weekly sync") opens on its Transcript tab with the turn of that time highlighted; the results stay in the list |

## Expected Final State
The moment was read in the right panel with its context, and its call opened at that moment.
