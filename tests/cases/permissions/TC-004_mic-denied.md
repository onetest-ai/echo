---
id: TC-004
title: Microphone denied: the others are still recorded
priority: high
type: functional
requirements: [F-SET-17, F-REC-15]
module: permissions
run: person
platform: macos
tags: [permissions, capture]
external_id: TC-PERM-004
---

# TC-004: Microphone denied: the others are still recorded

**Module:** Permissions | **Priority:** High | **Type:** Functional

## Preconditions
- `tccutil reset All ai.onetest.echo`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Deny Microphone, allow System Audio Recording | Health ⚠ shows the microphone missing |
| 2 | Record a short call | The others are recorded and transcribed; your side is empty |

## Expected Final State
One missing track doesn't lose the other.
