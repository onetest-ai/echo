---
id: TC-014
title: A client with no thread shows empty tabs, a New thread token and a Remove button, and its calls are offered in Found for you
priority: medium
type: functional
requirements: [F-CLIENT-01, F-CLIENT-04, F-CLIENT-06, F-THR-18]
module: clients
run: agent
platform: any
tags: [clients, empty, threads, found]
---

# TC-014: A client with no thread shows empty tabs, a New thread token and a Remove button, and its calls are offered in Found for you

**Module:** Clients | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No client or thread has been added or removed since the data was built

## Test Data

| Field | Value |
|---|---|
| Client | Duff (duff.example) |
| Threads | none |
| Calls | Duff discovery and Duff follow-up, in no thread (the Found for you card) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#projects` and read the group "Duff" | The name "Duff" has no thread row and no fold arrow |
| 2 | Click the name "Duff" | The route is `#client/duff/overview`; the head says "Duff", the tag "client" and "0 threads · duff.example" |
| 3 | Read the Overview | The block "About Duff" says "Nothing yet — who they are, what you do for them, who matters."; the section "Threads" says "No threads for this client yet — add one on Profile." |
| 4 | Click the tab "Calls" | "No calls in this client's threads yet." (the tab has no number) |
| 5 | Click the tab "Tasks" | Four piles with 0 and "Nothing here"; no button "Others" |
| 6 | Click the tab "People" | The text "Nobody in this client's threads yet" and "People join as its calls come in, or add someone with + Person."; there is no "+ Person" button on a client page |
| 7 | Click the tab "Profile" | The row "Threads" holds the token "+ New thread"; the row below has the button "Remove this client…" (not pressed) |
| 8 | Navigate to `{{base_url}}/#found` | A card "Duff calls" with the Client select on "Duff" and 2 calls: the client's calls are candidates for a thread |

## Expected Final State
Nothing changed.

## Teardown
- None: the case only reads

## Notes
- The People text of a client page mentions "+ Person", a button that only a thread page has. Verify at run time.
- Extends `TC-006` (removing a client made for the test) to a client the data already has.
