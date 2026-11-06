---
id: TC-013
title: The window updates on its own when a recording ends
priority: medium
type: functional
requirements: [F-CALLS-18]
module: window
run: person
platform: macos
tags: [calls, recording]
external_id: TC-WIN-007
---

# TC-013: The window updates on its own when a recording ends

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Echo running, a Teams call being recorded, the window open on Calls; another call's transcript open and scrolled

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | End the Teams call | Within seconds the new call is listed and its state moves on (transcribing, then ready) without a reload |
| 2 | Look at the other call you had open | Its scroll position, selected text and tab are as you left them |

## Expected Final State
New calls and their state appear live, and a refresh never resets what you were reading.
