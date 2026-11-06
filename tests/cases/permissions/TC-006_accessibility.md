---
id: TC-006
title: Accessibility enables mute sync and speaker names; without it recording still works
priority: high
type: functional
requirements: [F-SET-17, F-REC-10]
module: permissions
run: person
platform: macos
tags: [permissions, accessibility]
external_id: TC-PERM-006
---

# TC-006: Accessibility enables mute sync and speaker names; without it recording still works

**Module:** Permissions | **Priority:** High | **Type:** Functional

## Preconditions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Health → "Ask for permissions…"; enable Echo under Accessibility | Echo is listed there |
| 2 | Record a call and mute in Teams | Mute sync works and remote speakers get names |
| 3 | Turn Accessibility off and record again | Recording works; only mute sync and names are missing |

## Expected Final State
Accessibility adds features without being required for recording.
