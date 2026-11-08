---
id: TC-032
title: A transcribed call Echo leaves alone offers Write minutes, the model, the profile picker and the command
priority: high
type: functional
requirements: [F-MIN-06, F-MIN-12]
module: calls
run: agent
platform: any
data: standard
tags: [calls, minutes, auto, profile]
---

# TC-032: A transcribed call Echo leaves alone offers Write minutes, the model, the profile picker and the command

**Module:** Calls | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Call | auto.due |
|---|---|
| Duff discovery (`duff-discovery`) | before (it ended before automatic minutes were switched on) |
| Design guild (`design-guild`, the newest) | before |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" and click the row Duff discovery, then the tab "Minutes" | A box with the heading "No minutes yet" and the text "The transcript is ready." |
| 2 | Read the model line | "Claude Code writes them with Claude · work (sonnet), with your Claude account." |
| 3 | Open the select "Model for these minutes" | Five options: "Claude Code (as configured)", "Claude · work — sam@initech.example · initech" (selected), "Claude · other — sam.rivera@globex.example · globex", "Claude · out — not signed in", "Qwen · this computer"; close it without choosing |
| 4 | Read the buttons | "Write minutes" (primary; do not press it); "Or in a Claude Code session:" `/echo:process-call <folder>` with a Copy button |
| 5 | Run `browser_evaluate` with `() => { const b = [...document.querySelectorAll('.notes-actions .btn')].find((x) => x.textContent.trim() === 'Write minutes'); const r = b.getBoundingClientRect(), s = document.querySelector('[data-notesprofile]').getBoundingClientRect(); return { label: b.innerText.trim(), width: Math.round(r.width), cut: b.scrollWidth > b.clientWidth + 1, overlap: r.left < s.right && r.right > s.left && r.top < s.bottom && r.bottom > s.top } }` | `label` "Write minutes"; `width` at least 80; `cut` `false`; `overlap` `false`: the long Model picker never squeezes the button (#117: it was 26 px and read "mi"); the button may sit on the line under the picker |
| 6 | Click the row Design guild | The same heading "No minutes yet", the same model line and the "Write minutes" button |

## Expected Final State
Design guild offers Write minutes; nothing was started.

## Notes
- Creates or changes nothing. Choosing another model would pin it to the call: not done here.
- `call.profile` pinned missing or unreadable is not in the data (kept readable on purpose): "no longer exists — pick another" is not checked.
