---
id: TC-014
title: Record and stop from a chat
priority: medium
type: functional
requirements: [F-CC-09, F-REC-02, F-STT-02]
module: claude
run: person
platform: macos
tags: [record, plugin]
external_id: TC-CLA-014
---

# TC-014: Record and stop from a chat

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- No call going

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In a session type "record my next call in Ukrainian" | "Armed — recording starts when your next Teams call begins"; the menu-bar icon turns armed |
| 2 | Join a Teams call | Recording starts, pinned to Ukrainian |
| 3 | After a minute type "stop recording" | It stops; the transcript follows |

## Expected Final State
A session arms, pins the language and stops a recording in plain words.

## Notes
- Stays person-run: a session understanding "record my next call in Ukrainian" is the user's Claude Code session. What it runs is `echoctl arm --languages uk` / `echoctl stop`: `cli/TC-004` and `transcription/TC-004` run those with the test call.
