---
id: TC-028
title: Every select in the window opens Echo's menu — a line under each name, keys and clicks, the setting changed as before
priority: medium
type: functional
requirements: [F-WS-10]
module: window
run: agent
platform: any
data: standard
tags: [select, menu, profiles, keyboard]
---

# TC-028: Every select opens Echo's menu with a line under each name

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (`data: standard`, several AI profiles); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie)
- Viewport 1420x830

**Changes:** the data's settings (Where drafts go), put back in step 6

## Test Data

| Where | Holds |
|---|---|
| AI profiles | Claude Code (as configured), three Claude profiles (work, other, out — not signed in), Qwen · this computer |
| Settings → General | Where drafts go: Echo's default, Microsoft 365 Drafts, The default mail app, Another app… |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#settings`; look at "Where drafts go" | A quiet field with Echo's caret (a thin chevron), not the system's up-down arrows |
| 2 | Click the field | Echo's menu on the field, as the Mac's own (a listbox named "Where drafts go"): the four choices, "Echo's default" checked and lying right over the field, its text on the field's; no system list |
| 3 | Press ↓, then Enter | The menu closes; the field says "Microsoft 365 Drafts" and the row's text "Now: Microsoft 365 Drafts" (saved); the focus is on the field |
| 4 | Press Space, type `e`, press Escape | Space opens the menu, `e` marks "Echo's default"; Escape closes it with nothing changed (still Microsoft 365 Drafts), the focus on the field |
| 5 | Open the Claude panel (status bar → Claude); click its Profile field | Echo's menu on the field, inside the window (near the window's bottom it is moved up as far as it must): each profile's name with a line under it — "sonnet · as Claude Code is set up", "sonnet · its own Claude sign-in", "qwen3.8:27b-mlx · on this computer"; the panel's profile checked; click "Qwen · this computer" → the field says it, the menu closes |
| 6 | Open a call with no minutes yet (Calls → Globex standup → Minutes); click "Model"; click outside the menu | The same menu with the same lines; a click outside closes it, nothing changed. Put Where drafts go back to "Echo's default" (step 2's way) |
| 7 | Click the account chip in the top bar (Claude · work) | The chip unchanged and visible; Echo's menu opens **below** it, its right edge on the chip's, inside the window. Every profile is its name alone with one line under it — its model, then its account's address (no organisation, no plan), "not signed in", or where it runs (e.g. "sonnet · as Claude Code is set up", "qwen3.8:27b-mlx · on this computer"); the default checked. Under a line, "Manage profiles…". Escape closes it with nothing changed |
| 7a | Open the chip's menu again; click "Manage profiles…" | Settings → AI & minutes opens (`#settings/ai`); the default profile is unchanged (the chip reads as before) |
| 8 | Open `{{base_url}}/#project/acme-rollout/profile`; Calls → "+ Add"; pick "Subject contains" in "Kind of rule" from Echo's menu | The add form stays open with "Subject contains" chosen (a pick in the menu is no click outside the form) |
| 9 | `{{base_url}}/#project/acme-rollout/work` as a board: a column's ⋯ → "Cards in it are" → "waiting on someone", then back to "to do" | The column menu stays open through both picks; the select says what was picked |
| 10 | Open a task's details; click its Status value, then the Status select; pick "in progress" | The status reads "● in progress" (the field closes after the pick, not before it); put it back to "open" the same way |
| 11 | In the same task's details, click its "Who" field (the people list) | Each person with their face and name; what Echo knows of them is beside the name. A name is never cut for it: a long line is cut short with "…" (its whole text in the row's tooltip). Escape closes it, nothing changed |

## Expected Final State
Settings as before the run (Where drafts go: Echo's default).

## Notes
- Echo's menu replaces only the system's list: the select stays the page's state and its `change` runs as before; `data-native`, multiple and sized selects keep the system's. Rows, arrows, type-ahead and placement are the node test `select-menu.test.mjs`.
- A screenshot taken while the menu is open may close it (the window loses focus, as the system's list would).
