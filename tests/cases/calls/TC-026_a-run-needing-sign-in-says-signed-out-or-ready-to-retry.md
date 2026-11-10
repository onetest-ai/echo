---
id: TC-026
title: A run refused for sign-in says to sign in when the profile is signed out, and ready to try again when it is signed in
priority: high
type: functional
requirements: [F-MIN-09]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, run, signin]
---

# TC-026: A run refused for sign-in says to sign in when the profile is signed out, and ready to try again when it is signed in

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)
- Wait 10 seconds after the server starts: the sign-in checks of the AI profiles settle in the background

## Test Data

| Call | Profile | Heading | Text | Sign-in note | Buttons |
|---|---|---|---|---|---|
| Vendor call with Vandelay (`vendor-call-with-vandelay`) | Claude · out (signed out) | Claude Code needs you to sign in | Sign in to Claude (out) first | Claude · out isn't signed in. | Sign in…, Try again |
| Initrode intro (`initrode-intro`) | Claude · work (signed in) | Ready to try again | The last try said: Sign in to Claude first | Claude · work is signed in as sam@initech.example · initech now — click Try again. | Sign in again…, Try again |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Vendor call with Vandelay, then the tab "Minutes" | A box with the heading "Claude Code needs you to sign in" and the text "Sign in to Claude (out) first" |
| 2 | Read the sign-in note and the model select | The note "Claude · out isn't signed in." with a button "Sign in…" (do not press it); the select "Model for these minutes" shows "Claude · out — not signed in"; the button "Try again" |
| 3 | Click the row Initrode intro | A box with the heading "Ready to try again" and the text "The last try said: Sign in to Claude first" |
| 4 | Read the note and the buttons | "Claude · work is signed in as sam@initech.example · initech now — click Try again."; buttons "Sign in again…" (do not press it) and "Try again" |

## Expected Final State
Initrode intro offers Try again; no sign-in was started.

## Notes
- Creates or changes nothing. A profile signed in to another account (`Claude · other`) has no call needing it in the data, so "These minutes need another account" is not checked.
