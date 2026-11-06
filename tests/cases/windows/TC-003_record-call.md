---
id: TC-003
title: Record a Teams call on Windows
priority: critical
type: functional
requirements: [F-WIN-02]
module: windows
run: manual
platform: windows
tags: [windows, recording]
external_id: TC-WINOS-003
---

# TC-003: Record a Teams call on Windows

**Module:** Windows | **Priority:** Critical | **Type:** Functional

## Preconditions
- Windows 10 2004+ or 11, Teams (new) installed; the build is unsigned (updates are checked by SHA-256)
- Echo running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl procs` | `in call: false` |
| 2 | Tray → Record | The icon turns armed |
| 3 | Join a Teams test call; `echoctl procs` | An active session marked teams on your mic; `in call: true` |
| 4 | Talk 2 minutes both sides while a YouTube video plays; leave | Recording starts with the call and stops ~10 s after leaving (`call_started`, `recording_started`, `call_ended` in `.echo/events.jsonl`) |
| 5 | Read the transcript and the folder | Both sides; the mic track is the one Teams used; the video absent; no WAV files left |

## Expected Final State
A Teams call is recorded on Windows exactly like on macOS.
