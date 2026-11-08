---
id: TC-002
title: A thread's header shows client, stage, counts and tabs; Thread actions lists four items
priority: high
type: functional
requirements: [F-THR-08, F-THR-09]
module: threads
run: agent
platform: any
tags: [threads, header]
---

# TC-002: A thread's header shows client, stage, counts and tabs; Thread actions lists four items

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout (`acme-rollout`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/brief` | The heading "Acme rollout" is shown |
| 2 | Read the header | Tags "Acme" and "Delivery · Active"; the facts line reads "1 call · last call today" |
| 3 | Read the tabs | Five tabs: "Brief", "Work" with count 3, "Calls" with count 1, "People", "Profile" (read `data-tab-id`: brief, work, calls, people, profile); "Brief" is selected |
| 4 | Click "Thread actions" | A menu opens with exactly four items: "Open Acme", "Claude about this thread", "Sync now", "Delete…" |
| 5 | Press Escape | The menu closes (no menu item is visible); the URL is unchanged. Do not press "Claude about this thread" or "Sync now" (they start Claude Code) |

## Expected Final State
Acme rollout's header and tabs are as read, and the Thread actions menu is closed.

## Teardown
- None needed

## Changes
- Nothing: read only.

## Notes
- Step 3 reads the Work count on the Brief, before the Work tab has been opened: the work tree comes with the thread on every tab, as the call count does (#110).
