---
id: TC-033
title: A due the call didn't say (its own date taken as the due) is guessed — never late in Work, Tasks or Today, shown "from the call of <date>"; a due said in words, or set by the user, is late as before
priority: high
type: functional
requirements: [F-WORK-19, F-TASK-06, F-TODAY-26]
module: work
run: agent
platform: any
data: standard
tags: [work, ledger, due, late]
---

# TC-033: A guessed due is never late

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- As work/TC-032 (standard data, `$A`, **W**); step 1 of TC-032 done (T026 with `due_text` "today", which W never says; T027 with "Friday", which W says)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$A tree` | T026 `due` = W's day, `due_guessed` true; T027 `due` = the Friday after W, no `due_guessed` |
| 2 | `target/debug/echoctl project tasks` | In "I owe": T026 with `due` W's day, `late` false, `due_guessed` true, `due_from` `{"kind": "meeting", "date": <W's day>}`; T027 `late` true |
| 3 | Open `{{base_url}}/#project/acme-rollout/work`, T026's details | Due reads "<W's day> · from the call of <W's day>" (no "late"); T027's Due is its date, and its card is marked late |
| 4 | Open `{{base_url}}/#today` | My work lists T027 (late) and not T026 (its only due is guessed); the head's late count includes T027, not T026 |
| 5 | `$A edit '{"op": "edit", "node": "T026", "set": {"due": "<yesterday>"}}'`, reload Today | T026 now `late` (a due set by you), in My work, no `due_guessed` |

## Expected Final State
T026 with the user's due (late), T027 late.
