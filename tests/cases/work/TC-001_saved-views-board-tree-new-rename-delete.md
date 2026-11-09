---
id: TC-001
title: Saved views: Board and Tree, + New view, rename, duplicate, delete; the choice is remembered per thread
priority: medium
type: functional
requirements: [F-WORK-01]
module: work
run: agent
platform: any
tags: [work, views]
---

# TC-001: Saved views: Board and Tree, + New view, rename, duplicate, delete; the choice is remembered per thread

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme rollout has only the views Board and Tree

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout |
| New view name | Mine |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` | Two view buttons "Board" and "Tree" and "+ New view"; the board is shown |
| 2 | Click "Tree" | The tree is shown (a workstream header "ACME ROLLOUT · workstream"); "Tree" is the active view |
| 3 | Reload `{{base_url}}/` then navigate to the same URL | The Tree is still the active view (remembered) |
| 4 | Click "+ New view" | A third view button named "View 3" is active and its menu is open with an input named "View name" and the items "Duplicate view", "Delete view" |
| 5 | Replace the name with `Mine` and press Enter | The button reads "Mine" |
| 6 | Click "Mine" (the active view: a click opens its menu), then "Duplicate view" | A fourth view "Mine copy" exists and is active, and its menu is already open ("View name" holds `Mine copy`) |
| 7 | In that open menu click "Delete view" | "Mine copy" is gone; the view buttons are Board, Tree, Mine; Board is active (the first view) and no view menu is open |
| 8 | Click "Mine" (it becomes active), click it again to open its menu, and click "Delete view" | The view buttons are Board and Tree; Board is active and the board is shown (clicking the active "Board" would only open its menu) |

## Expected Final State
Acme rollout has its two default views; Board is active.

## Teardown
- Delete any extra view ("Mine", "Mine copy") as in steps 7-8; if "Board" is not active, click it (a click on the active view opens its menu: Escape closes it)

## Changes
- Adds and deletes saved views in `acme-rollout`'s board settings.
