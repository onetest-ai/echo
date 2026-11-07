---
id: TC-009
title: A prep shows only the sections it holds; an empty Expected says Nothing waits on you
priority: high
type: functional
requirements: [F-PREP-04]
module: prep
run: agent
platform: any
data: standard
tags: [prep, sections]
---

# TC-009: A prep shows only the sections it holds; an empty Expected says Nothing waits on you

**Module:** Prep | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Built less than 15 minutes ago: the data's times are relative to the build (automatic minutes come 15 minutes after a call; the meetings "live" at the build end 20 minutes after it)
- A prep's id is the one in `<dir>/manifest.json` → `preps` (stable for a given meeting id); Coming up today rows are `[data-up="<prep id>"]`
- The calls' Prep tab is reached by clicking the call's row, then the tab "Prep"

## Test Data

| Prep | In short | Expected from you | Last time | Open | New since | Who's coming |
|---|---|---|---|---|---|---|
| Globex sponsor update (Coming up today) | yes | 3 items | yes | yes | yes | 12 (ten + and 2 more) |
| Umbrella intro call (call) | yes | "Nothing waits on you." | yes | yes | yes | 4 |
| Globex retro (call, imported) | yes | "Nothing waits on you." | no | no | no | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync", click the row Umbrella intro call, then the tab "Prep" | Sections: In short, "Expected from you" with the grey text "Nothing waits on you.", "Last time · 30 Oct", "Open with these people", "New since then"; the side column "Who's coming" with 4 people |
| 2 | Click the row Globex retro, then the tab "Prep" | Sections: "In short" ("Globex retro: the SSO checklist is done, the questionnaire waits for legal, the access list is late.") and "Expected from you" with "Nothing waits on you." |
| 3 | Look for the other headings in Globex retro's prep | No "Last time", no "Open with these people", no "New since then", no "Who's coming", and not the sentence "Nothing to go on yet" |
| 4 | Read the footer of both preps | "Written HH:MM · sonnet"; no button "Write it again" (a recorded meeting's prep cannot be written again) |

## Expected Final State
Globex retro's prep shows two sections only.

## Notes
- Creates or changes nothing. The sentence "Nothing to go on yet: no earlier meeting with minutes, nothing open in its threads." shows only for a prep with no In short and no sections and no error: not in the data.
