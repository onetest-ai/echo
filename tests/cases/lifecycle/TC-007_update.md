---
id: TC-007
title: An update is offered and installed, keeping everything
priority: high
type: functional
module: lifecycle
run: manual
platform: macos
tags: [update]
external_id: TC-UPD-001
---

# TC-007: An update is offered and installed, keeping everything

**Module:** Lifecycle | **Priority:** High | **Type:** Functional

## Preconditions
- Echo installed and running
- A newer release on GitHub

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Start Echo (it checks at start, then every 6 hours) | "Update available" and a menu item |
| 2 | Menu → "Update to <version>…" → "Update" | Echo downloads, quits, replaces itself and starts on the new version (`echoctl --version`) |
| 3 | Check the data folder, models, permissions, calendar sign-in | All kept |

## Expected Final State
Updating is one click and loses nothing.
