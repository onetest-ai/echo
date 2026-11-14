---
id: TC-042
title: A recorded call's people are who joined, not who was invited, when Teams' participants list was read
priority: high
type: functional
requirements: [F-CALLS-19, F-CALLS-01, F-CALLS-05]
module: calls
run: agent
platform: any
data: standard
tags: [calls, header, people, teams]
---

# TC-042: A recorded call's people are who joined, not who was invited, when Teams' participants list was read

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`, and the run stops if `ECHOCTL_HOME` isn't inside `<dir>`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug)

## Test Data

| Call | Invite (but the user) | What Echo read of Teams' participants list (`speaking.jsonl`) |
|---|---|---|
| Acme security review (`acme-security-review`, yesterday) | Omar Haddad, Ivan Petrov | Omar Haddad in the call; Ivan Petrov "No response"; "Gomez, Rita (Acme)" in the call (not invited; Rita Gomez, rita@acme.example, in the people directory); the user's own tile and row |
| Acme weekly sync (`acme-sync-today`, today) | Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Yuki Tanaka, Jo Weber | none: the window wasn't read |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Before the window opens: `cat "$ECHOCTL_HOME"/calls/*-acme-security-review/meta.json` | It has no `participants` key yet |
| 2 | Navigate to `{{base_url}}/#calls` and wait for "Acme security review" | The list is shown |
| 3 | Read the row Acme security review: its `.row-faces` | Its `title` is "Omar Haddad, Rita Gomez" (no "Ivan Petrov", no "Sam Rivera"); two faces, no `+N` |
| 4 | Click the row Acme security review | The header's `.people` reads "Omar Haddad, Rita Gomez" |
| 5 | `cat "$ECHOCTL_HOME"/calls/*-acme-security-review/meta.json` | `"participants": ["Omar Haddad", "Gomez, Rita (Acme)"]`: who joined as Teams named them, the user left out; the other keys (title, started_at, ended_at) as before |
| 6 | Click the row Acme weekly sync (the first) | The header's `.people` reads "Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Yuki Tanaka, Jo Weber": the invite's people, as before; its `meta.json` has no `participants` key |

## Expected Final State
Acme security review's `meta.json` holds who joined; Acme weekly sync's is unchanged.

## Notes
- Step 5's write is the one change: Echo records who joined once, the first time the call is listed after it ended.
- Ivan Petrov stays on Acme security review's invite (`meeting.json`): threads and preps still see him; only the call's faces follow who joined.
