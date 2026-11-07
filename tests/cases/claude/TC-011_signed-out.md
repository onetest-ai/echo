---
id: TC-011
title: Claude Code signed out: the window says what to do, and Try again works after signing in
priority: high
type: functional
requirements: [F-MIN-09, F-AI-04]
module: claude
run: person
platform: macos
tags: [minutes, signin]
external_id: TC-CLA-010
---

# TC-011: Claude Code signed out: the window says what to do, and Try again works after signing in

**Module:** Claude Code | **Priority:** High | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- A transcribed call without minutes

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In a terminal: `claude` → `/logout` | — |
| 2 | Window → the call → Minutes → "Write minutes" | Within seconds "Claude Code needs you to sign in", what to do, a notification, "Try again" and "Show log" |
| 3 | `claude` → `/login`, then "Try again" | The minutes are written |

## Expected Final State
A signed-out Claude Code is explained and recovered from without losing the call.

## Notes
- Stays person-run: it signs a real Claude Code account out and in. The "needs you to sign in" state on test data is in the window cases of S-23 (a run waiting for a sign-in).
