---
id: TC-043
title: Today → Needs you now — Ignore (an eye crossed out beside ✦, and "Ignore: …" in ⌘K) takes the item off at once with nothing else (no task, no comment, no Claude), and it stays off after a reload
priority: high
type: functional
requirements: [F-TODAY-04]
module: today
run: agent
platform: any
data: hostile
tags: [today, needs-you-now]
---

# TC-043: Ignore takes an item off Needs you now for good

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Hostile data built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set hostile` (its `.echo/needs.json` has a Teams item and a mail item); the window served at `{{base_url}}`, 1201×868; CLI steps after `. <dir>/env`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Needs you now" | Two rows (`section.nd .nd-row`), each with Reply, ✦ and an eye crossed out named "Ignore — take it off Needs you now" (label and tooltip) |
| 2 | Click the first row's Ignore | That row leaves Needs you now at once; the other stays; no dialog, nothing under "Asks" |
| 3 | Reload the page | Still only the other row |
| 4 | `python3 -c "import json,os;n=json.load(open(os.environ['ECHOCTL_HOME']+'/.echo/needs.json'));print({k:(v['how'],v.get('note')) for k,v in n['decided'].items()})"` | One decision, the ignored item's id with `('ignore', None)`: no comment kept, no task made |
| 5 | Press `ControlOrMeta+k`, type `Ignore:`; then press Enter on NEEDS YOU NOW's "Ignore: <sender> — …" | ⌘K offers it for the remaining row only; after Enter the row leaves and Needs you now is gone from Today (nothing left in it) |

## Expected Final State
Both items decided `ignore`; Needs you now empty; nothing else changed (no task in any thread, no Asks line).

## Teardown
- `tests/fixtures/serve.sh down <dir>`; leave one blank tab

## Notes
- That a decision survives a read of Teams and mail which began before it (the read found the item again, then saved) is the Rust test `needs::a_decision_made_while_teams_and_mail_are_read_is_kept` — the bug where an item came back after Won't do; a test can't read Teams.
