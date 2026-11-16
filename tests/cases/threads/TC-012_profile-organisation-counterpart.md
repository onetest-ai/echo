---
id: TC-012
title: Profile → Organisations adds a counterpart organisation and × unlinks it
priority: medium
type: functional
requirements: [F-THR-13]
module: threads
run: agent
platform: any
tags: [threads, profile, organisations]
auto: [tests/auto/specs/S-05.spec.mjs]
---

# TC-012: Profile → Organisations adds a counterpart organisation and × unlinks it

**Module:** Threads | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has no counterpart organisation

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |
| Organisation | Globex |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/profile` | The Organisations row shows only "+ Organisation" and the hint "Who this thread is with besides its client: …" |
| 2 | Click "+ Organisation" | A select labelled "Organisation" with the options "Organisation…", "Globex · globex.example", "+ New organisation…" (no Acme: it is the client; no Initech: it is the user's company) |
| 3 | Choose `Globex · globex.example`. Wait up to 3 s after an action for the window to redraw before reading. | The Organisations row shows a token "Globex" with a button "Remove" (×) |
| 4 | Click the "Remove" button of the token "Globex". Wait up to 3 s after an action for the window to redraw before reading. | The token is gone; only "+ Organisation" remains |

## Expected Final State
Acme rollout has no counterpart organisation again.

## Teardown
- If step 4 was not reached: click the × of the token "Globex"

## Changes
- Links and unlinks Globex as a counterpart of `acme-rollout`.
