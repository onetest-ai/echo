---
id: TC-004
title: Only one Echo runs at a time
priority: high
type: functional
requirements: [F-LIFE-04]
module: lifecycle
run: person
platform: macos
tags: [lifecycle]
external_id: TC-LIFE-004
---

# TC-004: Only one Echo runs at a time

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open Echo again from Applications | At most the window opens |
| 2 | Run `echoctl app start` | Still one menu-bar icon and one `echoctl` app process |

## Expected Final State
A second start never makes a second app.
