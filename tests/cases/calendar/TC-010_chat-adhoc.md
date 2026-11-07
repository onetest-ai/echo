---
id: TC-010
title: An ad-hoc call's chat comes with the call
priority: medium
type: functional
requirements: [F-M365-03]
module: calendar
run: person
platform: macos
tags: [chat, m365]
external_id: TC-CHAT-002
---

# TC-010: An ad-hoc call's chat comes with the call

**Module:** Calendar | **Priority:** Medium | **Type:** Functional

## Preconditions
- A Microsoft 365 account whose organisation allows Echo's sign-in app (or its own app ID, TC-003)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Start a direct Teams call and record it | — |
| 2 | Send a chat message during it | — |
| 3 | A few minutes after, open the call → Chat | The message is there |

## Expected Final State
An ad-hoc call's chat is found and attached.
