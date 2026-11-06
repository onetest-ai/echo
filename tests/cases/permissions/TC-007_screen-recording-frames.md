---
id: TC-007
title: Shared-screen snapshots: opt-in, needs Screen Recording, one frame per change
priority: medium
type: functional
module: permissions
run: manual
platform: macos
tags: [permissions, frames]
external_id: TC-PERM-007
---

# TC-007: Shared-screen snapshots: opt-in, needs Screen Recording, one frame per change

**Module:** Permissions | **Priority:** Medium | **Type:** Functional

## Preconditions

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → Settings → tick "Save snapshots of shared screens" (off by default) | Without full Screen Recording: a "Screen Recording needed" notification |
| 2 | Allow full Screen Recording for Echo | — |
| 3 | Record a call where someone shares slides and changes them | The call has a Screens tab with about one frame per slide change, not one every few seconds |

## Expected Final State
Snapshots are opt-in and kept sparse.
