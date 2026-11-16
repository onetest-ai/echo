---
id: TC-002
title: Search filters the Calls list by title and by person
priority: high
type: functional
requirements: [F-CALLS-03]
module: window
run: agent
platform: any
tags: [calls, search]
external_id: TC-WIN-002
auto: [tests/auto/specs/S-01.spec.mjs]
---

# TC-002: Search filters the Calls list by title and by person

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}`

## Test Data

| Field | Value |
|-------|-------|
| Title search | Globex |
| Person search | Omar |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#calls` | Three calls are listed |
| 2 | Type `Globex` into "Search titles and people" | Only "Globex pilot kickoff" is listed |
| 3 | Clear the box and type `Omar` | Only "Acme weekly sync" is listed (Omar Haddad was invited to it) |
| 4 | Clear the box | All three calls are listed again |

## Expected Final State
Search narrows the list by a word of the title and by an attendee's name, and clearing it brings every call back.
