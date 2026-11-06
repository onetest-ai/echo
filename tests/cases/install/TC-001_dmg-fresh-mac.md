---
id: TC-001
title: Install from the DMG on a fresh Mac
priority: critical
type: functional
requirements: [F-LIFE-01]
module: install
run: person
platform: macos
tags: [install, gatekeeper]
external_id: TC-INST-001
---

# TC-001: Install from the DMG on a fresh Mac

**Module:** Install | **Priority:** Critical | **Type:** Functional

## Preconditions
- A fresh Mac account (tests/README.md → Test machines): no Echo settings, data folder or permissions
- The DMG downloaded with a browser from the GitHub release (so it carries the quarantine flag)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `Echo-<version>-macos-arm64.dmg` | It mounts with no "damaged" or "unidentified developer" warning |
| 2 | Drag Echo to Applications and open it | At most the standard "downloaded from the internet" prompt, naming Echo |
| 3 | Finish the welcome dialogs | No Dock icon stays; the octopus is in the menu bar |

## Expected Final State
A signed, notarised app installs and starts in the menu bar.
