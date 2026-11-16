---
id: TC-014
title: A memo's header says Memo with no kind chip; an imported call's says from Teams transcript
priority: medium
type: functional
requirements: [F-CALLS-05, F-CALLS-02]
module: calls
run: agent
platform: any
data: standard
tags: [calls, header, kind]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-014: A memo's header says Memo with no kind chip; an imported call's says from Teams transcript

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Header facts |
|---|---|
| Memo (`memo`) | "Memo", date, "4 min"; no kind chip; no people |
| Globex steering (`globex-steering`, imported) | date, "45 min", kind chip "Client" (not dashed), "from Teams transcript", 4 names |
| Acme weekly sync (Today) | date, "30 min", kind chip "Client", 6 names (the invite's 7 but the user), no "and N more"; no "Memo", no "from Teams transcript" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The list is shown |
| 2 | Click the row Memo | The header (`.facts`) holds "Memo" and "4 min" and a date with a weekday and a time; there is no `select[aria-label="Kind of meeting"]`; there is no `.people` |
| 3 | Click the row Globex steering | `.facts` holds "45 min", the chip "Client" (its `title` starts "From the meeting's invite"; it has no class `guess`) and the text "from Teams transcript" whose `title` is "Made from the meeting's Teams transcript: no audio, no live notes"; `.people` lists 4 names; no "Memo" |
| 4 | Click the row Acme weekly sync (the first) | `.facts` holds "30 min" and the chip "Client"; `.people` reads "Dana Lee, Omar Haddad, Rita Gomez, Ivan Petrov, Yuki Tanaka, Jo Weber" (no "Sam Rivera": the user; no "and N more") after 6 names; neither "Memo" nor "from Teams transcript" is there |

## Expected Final State
Acme weekly sync is open; its header shows no memo or import marks.

## Notes
- Creates or changes nothing.
