---
id: S-07
title: Clients and people: a client's page and tabs, a new thread and domains, a thread's people, a person's details
run: agent
data: small
cases:
  - tests/cases/clients/TC-008_own-company-is-your-company.md
  - tests/cases/clients/TC-001_open-a-client.md
  - tests/cases/clients/TC-004_profile-other-names-and-domains.md
  - tests/cases/clients/TC-005_new-thread-for-a-client.md
  - tests/cases/clients/TC-002_overview-about-and-thread-card.md
  - tests/cases/clients/TC-003_calls-and-tasks-tabs.md
  - tests/cases/clients/TC-007_client-with-threads-cannot-be-removed.md
  - tests/cases/clients/TC-006_remove-a-client-without-threads.md
  - tests/cases/clients/TC-009_organisations-that-are-not-clients-cli.md
  - tests/cases/people/TC-001_empty-thread-offers-people-from-calls.md
  - tests/cases/people/TC-003_list-shows-side-org-and-add.md
  - tests/cases/people/TC-004_map-drag-to-a-quadrant.md
  - tests/cases/people/TC-006_suggestions-yes-no-change.md
  - tests/cases/people/TC-005_person-panel-edit-in-place.md
  - tests/cases/people/TC-007_same-person-merge-and-split.md
  - tests/cases/people/TC-002_plus-person-adds-someone.md
  - tests/cases/people/TC-008_name-an-unknown-organisation.md
auto: [tests/auto/specs/S-07.spec.mjs]
---

# S-07: Clients and people: a client's page and tabs, a new thread and domains, a thread's people, a person's details

**Story:** Sam checks that Initech is his own company, opens Acme and Globex pages, edits their names and domains, makes a thread for a client, tries to remove clients, then builds a thread's people: seen people, a person added, the map, a person's details, suggestions and merging.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–17 |
| Globex's Profile open | stage 3, step 1 | 4 |
| The thread `people-lab` with the call Acme weekly sync | stage 10's precondition (two `echoctl` commands) | 10–17 (deleted in stage 17's Teardown) |
| Dana Lee in `people-lab` | stage 11, step 4 ("Add" in the List) | 12, 13, 14, 15 |
| people-lab's People tab open (Map in 10, 12–14; List in 11, 15–17) | stage 10, step 1 | 11–17 |

## Prelude
1. Load `{{base_url}}/` (the cookie).

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `clients/TC-008_own-company-is-your-company.md` | — | — |
| 2 | `clients/TC-001_open-a-client.md` | — | — |
| 3 | `clients/TC-004_profile-other-names-and-domains.md` | — | — |
| 4 | `clients/TC-005_new-thread-for-a-client.md` | — | On stage 3's view (Globex's Profile): skip the navigation of step 1, click "+ New thread" |
| 5 | `clients/TC-002_overview-about-and-thread-card.md` | — | — |
| 6 | `clients/TC-003_calls-and-tasks-tabs.md` | — | — |
| 7 | `clients/TC-007_client-with-threads-cannot-be-removed.md` | — | — |
| 8 | `clients/TC-006_remove-a-client-without-threads.md` | — | — |
| 9 | `clients/TC-009_organisations-that-are-not-clients-cli.md` | — | — |
| 10 | `people/TC-001_empty-thread-offers-people-from-calls.md` | — | Its precondition makes the shared thread `people-lab` (with Acme weekly sync). Skip the `project delete` of its Teardown (keep `people-lab` for stage 11) |
| 11 | `people/TC-003_list-shows-side-org-and-add.md` | — | Skip its `project create`/`include` precondition (`people-lab` from stage 10). On stage 10's view (people-lab's People, Map): skip the navigation of step 1, click "List". Skip its Teardown (keep `people-lab` for stage 12) |
| 12 | `people/TC-004_map-drag-to-a-quadrant.md` | — | Skip its `people-lab` and Dana Lee preconditions (`people-lab` from stage 10, Dana Lee added in stage 11). On stage 11's view (people-lab's People, List): skip the navigation of step 1, click "Map" (the view choice is kept in the browser). Skip its Teardown (keep `people-lab` for stage 13) |
| 13 | `people/TC-006_suggestions-yes-no-change.md` | — | Skip its `project create`/`include` precondition (`people-lab` from stage 10); run the sync and the people-apply. In step 1, first navigate to `{{base_url}}/#project/people-lab/brief`, then to `{{base_url}}/#project/people-lab/people`: the open People view doesn't pick up `people-apply` by itself, and navigating to the hash it already shows (or reloading it) doesn't either; leaving the tab and coming back does. Skip its Teardown (keep `people-lab` for stage 14) |
| 14 | `people/TC-005_person-panel-edit-in-place.md` | — | Skip its `people-lab` and Dana Lee preconditions (`people-lab` from stage 10, Dana Lee in it since stage 11). On stage 13's view (people-lab's Map): skip the navigation of step 1, click the chip "Dana Lee". Skip its Teardown (keep `people-lab` for stage 15) |
| 15 | `people/TC-007_same-person-merge-and-split.md` | — | Skip its `project create`/`include` precondition and the "Add" on Dana Lee (`people-lab` from stage 10, Dana Lee in it since stage 11); click "List", then "+ Person" `Dana L <dlee@acme.example>`. Then skip step 1's navigation and List click (List is pressed). Skip its Teardown (keep `people-lab` for stage 16) |
| 16 | `people/TC-002_plus-person-adds-someone.md` | — | Skip its precondition (`people-lab` from stage 10). On stage 15's view (people-lab's People, List): skip step 1; check on the snapshot that "List" is pressed. Skip its Teardown (keep `people-lab` for stage 17) |
| 17 | `people/TC-008_name-an-unknown-organisation.md` | — | Skip its precondition (`people-lab` from stage 10). On stage 16's view (people-lab's People, List): in step 1 skip the navigation and the click on "List"; do the "+ Person" part. Its Teardown deletes `people-lab` |

## Notes
- The people stages share one thread, `people-lab`, made once by stage 10's precondition and deleted by stage 17's Teardown; no people stage makes or deletes it. What one stage leaves is what the next needs, so the order matters: stage 10 reads the thread with nobody in it; stage 11 adds Dana Lee, whom stages 12–15 use (placed on the Map in 12, a champion through a suggestion in 13, edited in place in 14, merged with Dana L and split again in 15); stage 16 adds Pat Novak, stage 17 Lena Fox.
- A people stage whose kept state isn't there (an earlier stage failed) makes it as its own Preconditions say ("On its own") and still skips the `project delete` of its Teardown.
- The people view (Map, Org chart, List) is kept in the browser (`echo.people.view`), for every thread: stage 12 switches back to Map before reading it.
- Stage 13 comes before stage 14: a suggestion that a person already has the value is dropped ("already so"), so Dana Lee must not be a champion yet when stage 13 proposes it; stage 14 then sets her stance to supporter.
- Stage 17 creates organisation `contoso` and removes it; stage 9 uses `contoso-partners`.
- Stage 4 adds a thread under Globex and deletes it; stage 7 reads Acme, which still has Acme rollout.
- Stage 13 writes its suggestions with `/dev/stdin` in its preconditions.
