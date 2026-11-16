---
id: TC-018
title: echoctl attention lists the same open and done items as the Inbox
priority: medium
type: functional
requirements: [F-INBOX-08]
module: inbox
run: agent
platform: any
data: standard
tags: [inbox, cli]
---

# TC-018: echoctl attention lists the same open and done items as the Inbox

**Module:** Inbox | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Load `{{base_url}}/` and wait until the Inbox has the blocking item "Sign in to Claude · out" (the "Blocking" chip counts 1), up to 120 seconds, reloading the page if it hasn't come after 60: the sign-in check of the AI profiles settles in the background and the window shows it at its next attention refresh, 25–90 s in all (#513); before it, the answer lists 24 open items and none blocking
- Before any decision is taken in the window

## Test Data

| Kind | Open items |
|---|---|
| people | 10 |
| found | 6 |
| sender | 2 |
| signin | 1 |
| minutes | 1 |
| work | 1 |
| transcript | 1 |
| health | 1 |
| plugin-update | 1 |
| plugin | 1 |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `echoctl attention` | Exit code 0; JSON with `open` (25 entries), `done` (2), `urgent` 1 and `clients` mapping `acme` to "Acme", `duff` to "Duff", `cyberdyne`, `globex`, `initech`, `tyrell`, `umbrella`, `wonka` |
| 2 | Count `open` by `kind` | people 10, found 6, sender 2, and one each of signin, minutes, work, transcript, health, plugin-update, plugin |
| 3 | Read the first entry of `open` | The blocking one: `kind` "signin", `urgent` true, title "Sign in to Claude · out", an action "Sign in…" |
| 4 | Read `done` | Titles "Dana Lee: supporter → champion" and "New thread? “Old standup”" |
| 5 | Open the Inbox panel | "Open 25", "Blocking 1", "Done 2": the same numbers |

## Expected Final State
The CLI and the window agree.

## Teardown
- Click "Inbox" to close the panel

## Notes
- Creates or changes nothing.
- The sign-in item is in the CLI's answer only once the sign-in check has settled (after the window has loaded or about 10 seconds); before it settles, the answer has 24 open items and `urgent` 0.
