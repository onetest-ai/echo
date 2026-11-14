---
id: TC-034
title: On Yolo runs, a run with "Claude acts on its own" off whose send was refused shows its mail as a draft and its task's close as a change — never as a send — and "Prepared and asked"
priority: medium
type: functional
requirements: [F-TASK-15, F-TASK-12]
module: tasks
run: agent
platform: macos
data: standard
ai: virtual
tags: [tasks, yolo, yolo-runs, permissions, virtual-model]
---

# TC-034: Yolo runs shows a prepared draft, not a send

**Module:** Tasks | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-031's run `ask-asks-1` on the same data (Yolo on Globex pilot's T009 "Send the welcome mail" with "Claude acts on its own" off: its `m365 mail send` refused by Claude Code, the mail drafted with `compose`, T009 closed); `{{base_url}}/` loaded first (the cookie)

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#yolo`; read the row "Send the welcome mail" (`.yo-run[aria-label="Done: Send the welcome mail"]`) | "Done", "Prepared and asked"; the task as a link, "Globex pilot"; Claude's line "Drafted the welcome mail to Kim Park (virtual model); sending it was refused. Left for you: send the draft."; "Through Echo: 1 change" (no send, nothing refused: the send never reached Echo) and "Drafted A draft opened in your mail app — “Welcome to the Globex pilot” to kim@globex.example" |
| 2 | Tick "Changed or sent through Echo" | The row is still listed (closing T009 is a change) |
| 3 | Click its "Details" | "Changed `echoctl project work globex-pilot {"op":"edit","node":"T009","set":{"state":"done"}}`" and no "Sent" or "Tried to send" line; the log's first line says "runs with the folder session's: prepares and asks"; "✗ This command requires approval" after the `m365 mail send` step (refused by Claude Code itself) |

## Expected Final State
Nothing changed: the run is only read.

## Teardown
- None (S-33 stops the data)
