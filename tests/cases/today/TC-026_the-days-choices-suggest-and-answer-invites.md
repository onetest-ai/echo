---
id: TC-026
title: The day's choices — meetings you accepted at once with the one Echo suggests and why, invites still to answer with Accept / Maybe / Decline and Echo's suggestion; Keep one declines the others after a second click naming them; an answer goes to Microsoft 365 only on a click, and a failure says why
priority: high
type: functional
requirements: [F-TODAY-16]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, calendar, invites, m365]
---

# TC-026: The day's choices

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run (a busy evening: several meetings accepted at once, invites not answered): `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000
- The test data isn't signed in to Microsoft 365: an answer can't be sent, which is what step 6 checks. The send itself is checked on a real account (person-run, as S-20)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add core:calendar.choices --size L --above core:needs.now`; open `{{base_url}}/#today` | On top "The day's choices N": a line a choice in time order, each with its time |
| 2 | Read a line of meetings at once | Its time amber; "A or B or …" (the ones you accepted that overlap, not over); "You accepted N at once. Echo suggests <one> — <why>": you organise it, else in <thread>, else the longer, else the earlier / the first of them; a "Keep <subject>" button each, the suggested one filled |
| 3 | Read an invite's line | "<subject> not answered"; "Echo suggests: Decline — you go to <the meeting Echo suggests keeping then> then" when it overlaps where you go, else "Accept — your day is free then"; Accept, Maybe, Decline, the suggested one filled |
| 4 | Click a "Keep …" button | Nothing sent yet: "Decline <the others>? The organisers are told." with "Decline them" and Cancel |
| 5 | Click Cancel | The Keep buttons are back |
| 6 | On an invite click Accept | Its buttons disabled, the clicked one "Sending…"; a notice says it couldn't be sent and why ("not signed in to Microsoft 365 — `echoctl calendar login`"); the buttons come back; the invite still not answered (`echoctl today` unchanged, agenda unchanged) |
| 7 | Click a meeting's name on a line (or an invite's line) | Its card on the right, as from Your day |
| 8 | `target/debug/echoctl today reset`; reload | No choices widget (it isn't in the default) |

## Notes
- The rules are the node test `today-choices.test.mjs` (`dayChoices`, built on Your day's lines). What is sent is `m365::event_respond` (Rust test: the answer named, nothing sent without the click, an unknown answer or meeting refused); a run Echo started can't send (`not_in_run`).
