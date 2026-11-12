---
id: TC-041
title: My work shows a task that looks done — "Looks done — the call …" with Close and Not yet on its card — even when not due this week; Close there closes it as yours
priority: high
type: functional
requirements: [F-TODAY-26, F-WORK-18]
module: today
run: agent
platform: any
data: standard
tags: [today, my-work, looks-done]
---

# TC-041: My work shows a task that looks done

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- As work/TC-032 steps 1–2 done (T026 looks done from "Acme security review"; its only due guessed)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today` | My work has T026 "Send Dana the rollout summary" (not late, no date icon), its card saying "Looks done — the call Acme security review, <its day>: “Please send the summary and the action items after the call”" with "Close" and "Not yet" |
| 2 | Click the card's "Close" | The card goes from My work (T026 done); the task's details didn't open; `target/debug/echoctl project ledger acme-rollout tree`: T026 `done`, pinned `state` |

## Expected Final State
T026 done as the user's edit.
