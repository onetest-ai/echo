---
id: TC-002
title: Overview: About <client> is edited in place; each thread has a card with stories done, piles and calls
priority: high
type: functional
requirements: [F-CLIENT-02]
module: clients
run: agent
platform: any
tags: [clients, overview]
auto: [tests/auto/specs/S-07.spec.mjs]
---

# TC-002: Overview: About <client> is edited in place; each thread has a card with stories done, piles and calls

**Module:** Clients | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`; window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command
- Acme's About is empty

## Test Data

| Field | Value |
|-------|-------|
| Client | Acme |
| About | Pilot customer in Berlin. |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#client/acme/overview` | A card "About Acme" with "Edit" and the text "Nothing yet — who they are, what you do for them, who matters." |
| 2 | Read the section THREADS | One card: "Acme rollout", tag "Active", "1 call · last today", a progress line "0 of 1 story done", the tasks line "1 I owe" |
| 3 | Click "Edit", type `Pilot customer in Berlin.` and press the save shortcut: `Meta+S` (⌘S) on macOS, `Control+S` on Windows and Linux | The card shows "Pilot customer in Berlin." |
| 4 | Click "Edit", type `x` and press Escape | The text "Pilot customer in Berlin." is unchanged |
| 5 | Click the card's title "Acme rollout" | The URL ends with `#project/acme-rollout/` and a tab name |

## Expected Final State
Acme has the About text; the thread card is as read.

## Teardown
- Navigate back to the client, "Edit", clear the text, press the save shortcut (`Meta+S` on macOS, `Control+S` elsewhere); confirm "Nothing yet…"

## Changes
- Changes the About text of Acme.

## Notes
- The About editor saves on the platform's own modifier: ⌘ on a Mac, Ctrl elsewhere (`ui/app/*.js`, the `pj-brief` keydown: `MAC ? e.metaKey : e.ctrlKey`), like the other editors. On a Mac `Control+S` does nothing; that is intended, not a failure.
