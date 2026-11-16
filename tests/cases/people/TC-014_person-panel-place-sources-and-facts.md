---
id: TC-014
title: A person's panel shows their place in the thread, where each value comes from, their summary and their facts
priority: medium
type: functional
requirements: [F-PPL-05, F-PPL-06]
module: people
run: agent
platform: any
tags: [people, panel, sources, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-014: A person's panel shows their place in the thread, where each value comes from, their summary and their facts

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No value has been edited and no question answered since the data was built

## Test Data

| Person | Title / source | Place in Acme rollout | Who they are | Facts | Question |
|---|---|---|---|---|---|
| Dana Lee | Delivery manager · Microsoft 365 (also phone, Berlin office) | Role "delivery lead" (yours, Echo leaves it), stance champion, influence high, interest high, workstream SSO, governance yes | Echo's summary: "Dana Lee runs delivery on Acme's side and keeps the rollout on schedule." | 10: eight shown, the button "All 10" | none |
| Omar Haddad | Security lead · you | Role "security" (from a call, Echo's), stance sceptic, influence high, interest low, governance no | none: "Describe them…" | 3, no "All" button | Microsoft 365 says the title is "Head of IT security", you set "Security lead": Use theirs / Keep mine |
| Rita Gomez | no title | Role not set, stance supporter, influence low, interest high, workstream data migration, works for Dana Lee | In your words: "Rita owns the budget for the rollout and the renewal; she decides fast when she has numbers." | none | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/people`, press "List" and click the row of Dana Lee | The right panel shows "Dana Lee", "Delivery manager · Acme", the block "Who they are" with Echo's summary and the note "Echo's summary of what was said — click to write your own" |
| 2 | Read "In Acme rollout" in the panel | Role "delivery lead" with "yours (Echo leaves it)", Stance "champion", Influence and Interest with the word "high", Workstream "SSO", Governance "yes" |
| 3 | Read "About them" | Title "Delivery manager · Microsoft 365", Phone "+49 30 0000 0000 · Microsoft 365", Office "Berlin · Microsoft 365" |
| 4 | Read "What was said about them" and press "All 10" | Eight facts first, then ten; the button then reads "Fewer"; each fact has a quote and a call mark |
| 5 | Click the row of Omar Haddad | Role "security" with "from" a call (Echo's), Stance "sceptic", Governance "no"; the block "Who they are" offers "Describe them…"; the About block has the line "Microsoft 365 says title is “Head of IT security” — you set “Security lead”." with the buttons "Use theirs" and "Keep mine" (neither pressed) |
| 6 | Click the row of Rita Gomez | The note "In your words"; Workstream "data migration"; Works for here "Dana Lee"; Role "Set" (empty); Influence "low", Interest "high" |

## Expected Final State
The panel shows Rita Gomez; nothing was changed.

## Teardown
- Close the panel (`[aria-label="Close the panel"]`)

## Notes
- Reads only: `TC-005` edits in place and `TC-006` answers the suggestions of the same people.
- Covers person.place (stance, role by you or Echo, governance yes or no or unset, workstream, works for), person.field_source (you, Microsoft 365, Echo, a question), person.about (Echo's, the user's, none) and person.facts (0, up to 8, more than 8).
