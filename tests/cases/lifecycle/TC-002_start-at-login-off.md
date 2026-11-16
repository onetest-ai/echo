---
id: TC-002
title: With start at login off, Echo doesn't start
priority: low
type: functional
requirements: [F-LIFE-02]
module: lifecycle
run: person
platform: any
tags: [lifecycle, login]
external_id: TC-LIFE-002
---

# TC-002: With start at login off, Echo doesn't start

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- Echo installed and running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Untick "Start Echo at login"; log out and in | Echo does not start |
| 2 | Open it by hand | It starts |

## Expected Final State
The setting is honoured both ways.

## Notes
- Stays person-run: it logs the user out and in, and "Start at login" writes `~/Library/LaunchAgents/ai.onetest.echo.plist`, the user's own Echo's LaunchAgent on a Mac where Echo is installed.
