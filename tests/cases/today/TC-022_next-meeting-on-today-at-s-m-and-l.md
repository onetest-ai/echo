---
id: TC-022
title: Next meeting on Today — S the meeting and its buttons, M what is expected from you, L its prep brought to the front (walk in knowing, who's coming, what to ask)
priority: high
type: functional
requirements: [F-TODAY-13]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, next-meeting, prep]
---

# TC-022: Next meeting on Today at S, M and L

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`
- `target/debug/echoctl widget show core:calendar.next` names a meeting (`next`), and the meeting has a prep (`next.prep.ready`); if it hasn't, give it one for this case: copy any `<dir>/home/prep/<id>/prep.json` whose `summary`, `expected` and a question in `open` are set to `<dir>/home/prep/<next.prep.id>/prep.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add core:calendar.next --size L --above core:needs.now`; open `{{base_url}}/#today` | On top, across: "Next · <time> · in N min" (or "Now · since <time>" for one under way), its thread's chip, its subject; "N people · records when it starts" (when it is set to record); **Walk in knowing**: the prep's In short and up to 3 things expected from you ("You owe: …", "<name> waits for: …", late in red); Open prep, Join & record with the language beside it (a Teams meeting not over), Ask Claude about it; on the right **Who's coming** (initials; a hover says the name and the answer; a declined one struck through; a newcomer named), **Ask them** (up to 2 open questions), "● Recording starts with the call" (when set to record) |
| 2 | Click Open prep | The prep opens (as Show prep in Your day) |
| 3 | Back on Today, click Ask Claude about it | The Claude panel opens to type "Help me get ready for the meeting “<subject>”: read its prep with `echoctl read echo://prep/<id>` …" — as My work's Work on it does; with the test data's fake Claude Code it says "Claude didn't start — send it again once the panel shows Claude." |
| 4 | `target/debug/echoctl today size core:calendar.next M`; reload | A card: the time line, the subject, "Expected from you: …" (the same things, on one line), Show prep (and Join & record) — no Who's coming, no Ask them |
| 5 | `… size core:calendar.next S`; reload | The time line, the subject and its buttons only |
| 6 | `target/debug/echoctl today reset` | Next meeting gone from Today (it isn't in the default yet) |

## Notes
- None left today → no widget and no gap: test data always has meetings after now, so the Rust test `widgets::next_is_the_first_meeting_not_over_and_none_once_the_day_is_done` and the node test `today-layout.test.mjs` (an empty widget takes no space) hold it.
- Phase 4 of the Today widgets (#315). A meeting with no prep shows Prepare (or "Writing the prep…") and no Walk in knowing / Ask them. The data is the prep's and the invite's — no model is asked for this widget.
