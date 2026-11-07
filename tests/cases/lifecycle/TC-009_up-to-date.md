---
id: TC-009
title: On the newest version nothing is offered
priority: low
type: functional
requirements: [F-LIFE-07]
module: lifecycle
run: person
platform: macos
tags: [update]
external_id: TC-UPD-003
---

# TC-009: On the newest version nothing is offered

**Module:** Lifecycle | **Priority:** Low | **Type:** Functional

## Preconditions
- The newest release installed

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Start Echo; wait a minute | No Update item, no notification |

## Expected Final State
No false update offers.

## Notes
- Stays person-run: the check's result is shown only by the tray and a notification (`app::check_updates`, `update::AVAILABLE` in the app process); the served window can't show it. (2026-11-07: v0.8.2 is both the build and the latest release.)
