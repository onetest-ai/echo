---
id: TC-233
title: Enter in the Calls box lists titles and people, then what was said by source with its moments and the words marked
priority: high
type: functional
requirements: [F-CALLS-20]
module: calls
run: agent
platform: any
tags: [calls, search, recall]
---

# TC-233: Enter in the Calls box lists titles and people, then what was said by source

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --set standard`, served at `{{base_url}}`
- Recall on and indexed: after `set -a; . <dir>/env; set +a`, with `ECHOCTL_CONFIG_DIR` checked to be inside `<dir>`
  (else stop), `target/debug/echoctl brain set --enabled true`, then `target/debug/echoctl brain sync` (exit 0 both);
  the page loaded again after it (`{{base_url}}/`)

## Test Data

| Field | Value |
|-------|-------|
| Words | security review |
| The call they name | Acme security review (yesterday, Acme rollout, client Acme) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls`, type `security review` into the search box and press Enter | Under the box: four kind icons (Calls, Mail, Teams channels, Documents — their tooltips), a filter icon "Filters" and a count "1 call · N sources". The list's first group is "Titles and people · 1" with the row "Acme security review" (its day, "Yesterday", not a time); below it the group "Said in calls, mail, channels · N" |
| 2 | Read the first source row of "Said in calls, mail, channels" | "Acme security review", with a calls icon, "Yesterday" and a time, "Acme", "3 moments"; it is open (its chevron turned): three moments, each with "Transcript", a speaker and a time (m:ss), and two lines of what was said with "security" or "review" in a violet mark |
| 3 | Read the other source rows | Each has its kind's icon, title, day, and "1 moment" (or more); they are closed; when there are more than six sources the list ends with "N more sources" |
| 4 | Click the closed row "Analyst briefing" (or the second source row), then click it again | Its moment shows under it, then folds away again |
| 5 | Click "N more sources" | The rest of the sources are listed; the link is gone |
| 6 | Type ` x` at the end of the box (no Enter) | The groups go: the list is the calls whose title or people hold the words (here none: "Nothing matches") while the chips row stays |
| 7 | Press Escape in the box | The box is empty, the chips row is gone, the placeholder reads "Search titles and people" and every call is listed by day again |

## Expected Final State
Enter searched what was said: titles and people first, then the sources with their moments; Escape left the search.
