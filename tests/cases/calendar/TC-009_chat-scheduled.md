---
id: TC-009
title: A recorded meeting's chat comes with the call
priority: high
type: functional
module: calendar
run: manual
platform: macos
tags: [chat, m365]
external_id: TC-CHAT-001
---

# TC-009: A recorded meeting's chat comes with the call

**Module:** Calendar | **Priority:** High | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)
- A scheduled Teams meeting set to record

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Before it starts, post a message in its chat | — |
| 2 | During the call, post a link and a file | — |
| 3 | Ten minutes after it ends, post another message | — |
| 4 | Open the call → Chat | Messages from about 2 minutes before the start to 2 after the end, with authors and times; the file by name; the late message is not there |
| 5 | Check the message with a link | Plain text: no formatting or script runs |

## Expected Final State
The call holds its meeting chat within its time window, as plain text.
