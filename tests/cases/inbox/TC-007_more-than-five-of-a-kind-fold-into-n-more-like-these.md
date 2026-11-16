---
id: TC-007
title: More than five items of a kind fold into "N more like these"; five or fewer do not
priority: high
type: functional
requirements: [F-INBOX-03]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, fold]
---

# TC-007: More than five items of a kind fold into "N more like these"; five or fewer do not

**Module:** Inbox | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), up to 120 seconds, reloading the page if it hasn't come after 60: the sign-in check of the AI profiles settles in the background and the window shows it at its next attention refresh, 25–90 s in all (#513); before it, the answer lists 24 open items and none blocking

## Test Data

| Group · kind | Items | Shown | Button |
|---|---|---|---|
| Acme · people | 10 | 5 | "5 more like these" |
| Everything else · found | 4 | 4 | none |
| Acme · sender | 2 | 2 | none |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day" and click "Inbox" | The right panel opens on the tab "Inbox"; the chips "Open", "Blocking" and "Done" are shown |
| 2 | Count the people questions under "Acme" (titles ending with ?) | Five rows; the first is "Is Rita Gomez a champion?" |
| 3 | Read the button after the fifth | "5 more like these" (`.ny-more`) |
| 4 | Click "5 more like these" | Ten people questions are listed (adds "Add Max Ritter (Acme)?…", "Ivan Petrov reports to Omar Haddad?", "Has Yuki Tanaka left?", "Omar Haddad is Head of security operations?", "Acme partner Vandelay?"); the button is gone; the Acme count is still 15 |
| 5 | Look under "Everything else" for "more like these" | None: the four found items and the other kinds show in full |

## Expected Final State
The people questions are all shown.

## Teardown
- Click "Inbox" to close the panel (the fold resets on reload)

## Notes
- Creates or changes nothing.
