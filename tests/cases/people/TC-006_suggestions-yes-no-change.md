---
id: TC-006
title: Suggestions from the people pass are answered Yes, No or Change…; three show and the rest wait
priority: medium
type: functional
requirements: [F-PPL-06]
module: people
run: agent
platform: any
tags: [people, suggestions]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-006: Suggestions from the people pass are answered Yes, No or Change…; three show and the rest wait

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Create the scratch thread with the Acme call: `echoctl project create "People lab"` (id `people-lab`), then `echoctl project include people-lab <folder of Acme weekly sync>` (the entry of `$ECHOCTL_HOME/calls` ending `-acme-weekly-sync`). People are placed per thread, so the case never touches Acme rollout
- In S-07: `people-lab` is the scenario's shared thread, made by stage 10's precondition (people/TC-001) and kept (Dana Lee in it and placed, not yet a champion: a suggestion of a value the person already has is dropped); skip making it. On its own: make it as above
- The people view is Map (click "Map" in the view switch if another is pressed)
- `echoctl project ledger people-lab sync`
- Propose three changes: `echoctl project ledger people-lab people-apply --evidence <id from sync's pending list> --file /dev/stdin` with the standard input `[{"who":"Dana Lee","field":"stance","value":"champion","quote":"We need the SSO checklist from you by the end of today."},{"who":"Omar Haddad","field":"influence","value":3,"quote":"Omar will answer the security questionnaire by Friday."},{"who":"Omar Haddad","field":"title","value":"Security lead","quote":"Omar will answer the security questionnaire by Friday."}]` → `added: 3`

## Test Data

| Field | Value |
|-------|-------|
| Thread | People lab (`people-lab`), with the call Acme weekly sync |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/people-lab/people` | Three rows above the map: "Is Dana Lee a champion? “We need the SSO checklist from you by the end of today.” Acme weekly sync · today · 00:13"; "Omar Haddad's influence: high? …"; "Omar Haddad is Security lead? …" — each with "Yes", "No", "Change…" |
| 2 | Click "Yes" on the first row. Wait up to 3 s after an action for the window to redraw before reading. | The row is gone; `echoctl project people people-lab` shows Dana Lee in the thread with `"stance": "champion"` |
| 3 | Click "No" on the row about Omar's influence. Wait up to 3 s after an action for the window to redraw before reading. | The row is gone; Omar Haddad has no `influence` in `places` |
| 4 | Click "Change…" on the row "Omar Haddad is Security lead?" | The row reads "Omar Haddad is Security lead →" with an input "Your version" holding `Security lead`, and "Save", "Cancel" |
| 5 | Replace the text with `Head of Security`, click "Save". Wait up to 3 s after an action for the window to redraw before reading. | No suggestion row is left; `echoctl project people people-lab` shows Omar's `"title": "Head of Security"` |

## Expected Final State
All three suggestions are answered.

## Teardown
- `echoctl project delete people-lab`

## Changes
- Adds three suggestions to `people-lab` and answers them.

## Notes
The "N more from the calls" button (more than three suggestions) is not covered.
