---
id: TC-010
title: A card opens the Details drawer; the title and the description are edited in place
priority: high
type: functional
requirements: [F-WORK-09]
module: work
run: agent
platform: any
tags: [work, details]
---

# TC-010: A card opens the Details drawer; the title and the description are edited in place

**Module:** Work | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Task | T001 Send Acme the SSO checklist |
| Description | Checklist covers SAML and SCIM. |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and click the card T001 | The right panel opens on the tab "Details": `T001`, the tag "task", a button "Send to Claude" (do not press it) and "Edit", the title "Send Acme the SSO checklist", the path "Acme rollout › SSO for the Berlin pilot", the text "No description" with "Add" |
| 2 | Click "Add", type `Checklist covers SAML and SCIM.` and press ⌘S (Ctrl+S) | The description text is shown; History gets an entry "you set description" |
| 3 | Click "Edit", replace the title with `Send Acme the SSO checklist (v2)` and press Enter | The heading of the panel reads "Send Acme the SSO checklist (v2)"; the card on the board shows the same title |
| 4 | Click the description, change nothing and press Escape | The text is shown unchanged |

## Expected Final State
T001 has a description and a new title.

## Teardown
- Set the title back with "Edit" to `Send Acme the SSO checklist`; click the description, clear it and press ⌘S

## Changes
- Changes title and description of T001.
