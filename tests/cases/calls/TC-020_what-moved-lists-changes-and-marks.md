---
id: TC-020
title: What moved lists what the call changed in its thread, hides what the verifier called wrong, and marks the doubtful
priority: high
type: functional
requirements: [F-MIN-04]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, what-moved]
---

# TC-020: What moved lists what the call changed in its thread, hides what the verifier called wrong, and marks the doubtful

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | Thread | Rows shown (6) |
|---|---|---|
| Acme weekly sync (Today) | Acme rollout | Criterion met S001 · Done T001 "Send the SSO checklist" · Held T007 "Answer the security questionnaire" (mark "weak · to review") · T005 "Hold the help desk training session" (mark "to check") · Answered Q001 · Now waits T003 "Prepare the training deck" |
| Not shown |  | T008 "Run the data migration dry run" (the verifier called it wrong) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Acme weekly sync, then open What moved with the header button `[data-panel-flip="moved"]` (beside My notes; skip the click if the right panel already shows What moved) | The view "What moved" in the right panel (`#r-right .mv`) has the thread "Acme rollout" with "6 changes"; the header button and the panel tab both carry the count 6 |
| 2 | Read the six rows | Each row has a label ("Criterion met", "Done", "Held", "Now waits", "Answered" and one for T005), the item id and title, the words said in quotes with the speaker, and a time chip |
| 3 | Look for "Run the data migration dry run" in the What moved view | It is not there |
| 4 | Read the mark of the row "Answer the security questionnaire" | "weak · to review" |
| 5 | Read the mark of the row "Hold the help desk training session" | "to check" |
| 6 | Read the row "Send the SSO checklist" | Label "Done" and no mark |

## Expected Final State
What moved shows six changes for Acme rollout, two of them marked.

## Notes
- Creates or changes nothing. Pressing a time chip or an item opens the Transcript / Details; not done here.
- The mark "kept yours" needs a change the user overrode: not on this call.
