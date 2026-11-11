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
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-026: The day's choices

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run (a busy evening: several meetings accepted at once, invites not answered): `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1000
- Run steps 1–3 within 15 minutes of the build: the meetings under way end 20 minutes after it, and a line of meetings at once is no choice once they are over. The invite "Design system walkthrough" lies in the free time between the meetings under way (they end 20 minutes after the build) and the ones to come (37 minutes after), from 22 to about 36 minutes after the build, at any hour of the build; once it is over it is no choice any more
- The test data isn't signed in to Microsoft 365: an answer can't be sent, which is what step 6 checks. The send itself is checked on a real account (person-run, as S-20)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#today` | "The day's choices 5" (on Today by default, beside Your day): a line a choice in time order, each with its time |
| 2 | Read a line of meetings at once | Its time amber; "A or B or …" (the ones you accepted that overlap, a chain counting, not over; a call is no choice); "You accepted N at once. Echo suggests <one> — <why>": you organise it (only when exactly one of them is yours), else in <thread> (only when exactly one is in a thread), else the longer, else the earlier / the first of them (a tie); a "Keep <subject>" button each, the suggested one filled |
| 3 | Read the invites' lines | Each "<subject> not answered" with Accept, Maybe, Decline, the suggested one filled. One overlaps where you go: "Acme training dry run not answered", "Echo suggests: Decline — you go to Initech architecture board then", Decline filled. One is at a free time: "Design system walkthrough not answered", "Echo suggests: Accept — your day is free then", Accept filled, its time between the two lines of meetings at once |
| 4 | Click (or Tab to and press Enter on) a "Keep …" button that isn't the suggested one; two of the same name at once are numbered ("Vendor demo (1)", "(2)") | Nothing sent yet: "Decline <the others>? The organisers are told." with "Decline them" and Cancel; one you organise among the others is not declined: "Yours — <subject> — stays: cancel it in Outlook." |
| 5 | Click Cancel | The Keep buttons are back; the keyboard stays in the line (after Keep on "Decline them", after Cancel on that Keep) |
| 6 | On an invite click Accept | Its buttons disabled, the clicked one "Sending…"; a notice says it couldn't be sent and why ("not signed in to Microsoft 365 — echoctl calendar login", the command as code, no backticks); the buttons come back; the invite still not answered (`echoctl today` unchanged, agenda unchanged) |
| 7 | Click a meeting's name on a line (or an invite's line) | Its card on the right, as from Your day |
| 8 | `target/debug/echoctl today remove core:calendar.choices`; reload; then `today reset`; reload | Gone after the remove; back after the reset (it is in the default) |

## Notes
- The rules are the node test `today-choices.test.mjs` (`dayChoices`, built on Your day's lines). What is sent is `m365::event_respond` (Rust test: the answer named, nothing sent without the click, an unknown answer or meeting refused); a run Echo started can't send (`not_in_run`).
