---
id: TC-009
title: No on a people question takes the item out of Open and puts it in Done
priority: high
type: functional
requirements: [F-INBOX-04, F-INBOX-06, F-INBOX-07]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, people, decision]
---

# TC-009: No on a people question takes the item out of Open and puts it in Done

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), within 10 seconds, no reload: the sign-in check of the AI profiles settles in the background and the window reads the Inbox again as soon as it does (#513); before it, the answer lists 24 open items and none blocking
- The people questions of Acme are folded: "5 more like these" has not been pressed since the page loaded (the fold stays open until a full load). In S-24 `inbox/TC-007` opens it earlier: the scenario reloads the page before this case

## Test Data

| Field | Value |
|---|---|
| Item | Is Rita Gomez a champion? (Acme · people, first of ten) |
| Counts before | on fresh data Open 25, Done 2, Acme 15, people 10; read them in step 1 (earlier decisions in a scenario lower Open and Acme and raise Done) |
| Counts after | one less Open and Acme, one more Done; people 9 ("4 more like these") |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox"; note the numbers on the chips "Open" (N) and "Done" (D) and on the header "Acme" (A) | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown (on fresh data N = 25, D = 2, A = 15) |
| 2 | Click "No" on "Is Rita Gomez a champion?" | The item shows "Working…" or is already gone |
| 3 | Wait up to 5 seconds | "Is Rita Gomez a champion?" is no longer under Acme; the chip reads "Open N−1"; the Acme header A−1; the fold button under the people reads "4 more like these" |
| 4 | Click the chip "Done" | "Done D+1"; the first row is "Is Rita Gomez a champion?" (struck through) with "today" |
| 5 | Run `echoctl attention` | `open` has N−1 items (no key for that question); `done` has D+1 entries and the first title is "Is Rita Gomez a champion?" |

## Expected Final State
The question is decided and listed under Done.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Writes the decision to the thread's people suggestions (`people_decide`); no Claude Code, no network. The data is disposable: it has no undo here.
