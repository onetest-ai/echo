---
id: TC-001
title: A client opens from Thread actions or the Threads list, on tabs Overview, Calls, Tasks, People, Profile
priority: high
type: functional
requirements: [F-CLIENT-01]
module: clients
run: agent
platform: any
tags: [clients]
---

# TC-001: A client opens from Thread actions or the Threads list, on tabs Overview, Calls, Tasks, People, Profile

**Module:** Clients | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Field | Value |
|-------|-------|
| Client | Acme (`acme`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/brief`, click "Thread actions" and the item "Open Acme" | The URL ends with `#client/acme/overview`; the heading "Acme" with the tag "client"; the facts line "1 thread · acme.example"; a button "Claude about Acme" (do not press it) |
| 2 | Read the tabs | "Overview", "Calls" with count 1, "Tasks" with count 1, "People", "Profile" (`data-pj="ctab"`, `data-tab-id` overview, calls, tasks, people, profile); "Overview" is selected |
| 3 | Click the name "Globex" in the left list (`[data-pj="client"][data-id="globex"]`) | The URL ends with `#client/globex/overview`; heading "Globex"; facts "1 thread · globex.example" |

## Expected Final State
The Globex client page is open.

## Teardown
- None needed

## Changes
- Nothing: read only.
