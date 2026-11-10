---
id: S-26
title: Ten threads in every stage: membership, people and clients
run: agent
data: standard
cases:
  - tests/cases/threads/TC-018_list-on-the-full-set-groups-stages-and-closed-fold.md
  - tests/cases/threads/TC-019_calls-tab-says-why-each-call-is-in.md
  - tests/cases/threads/TC-020_one-call-in-two-threads-with-its-own-reason-in-each.md
  - tests/cases/threads/TC-024_a-suggestion-never-assigns-a-call.md
  - tests/cases/threads/TC-022_taken-out-calls-stay-out-and-are-counted.md
  - tests/cases/threads/TC-026_profile-rules-and-match-counts-on-ten-threads.md
  - tests/cases/threads/TC-027_counterparts-none-one-and-several.md
  - tests/cases/threads/TC-025_found-for-you-six-candidates-and-their-client-choices.md
  - tests/cases/threads/TC-023_closed-thread-keeps-its-calls-and-takes-no-new-ones.md
  - tests/cases/people/TC-009_map-places-people-and-organisations-by-influence-and-interest.md
  - tests/cases/people/TC-010_list-splits-people-in-the-thread-from-those-seen-in-calls.md
  - tests/cases/people/TC-012_new-tag-on-people-first-seen-within-two-weeks.md
  - tests/cases/people/TC-013_client-people-tab-threads-column-and-the-user-s-company.md
  - tests/cases/people/TC-014_person-panel-place-sources-and-facts.md
  - tests/cases/people/TC-011_list-side-your-company-an-organisation-a-domain-or-outside.md
  - tests/cases/clients/TC-010_an-account-is-a-client-an-organisation-is-not.md
  - tests/cases/clients/TC-011_profile-other-names-and-domains-of-five-clients.md
  - tests/cases/clients/TC-012_about-and-logo-present-and-absent.md
  - tests/cases/clients/TC-013_a-client-with-only-closed-threads.md
  - tests/cases/clients/TC-014_a-client-with-no-threads.md
  - tests/cases/clients/TC-017_the-user-s-company-with-a-thread.md
  - tests/cases/clients/TC-015_overview-cards-of-a-client-with-open-and-closed-threads.md
  - tests/cases/clients/TC-016_calls-and-tasks-tabs-of-a-client-with-three-threads.md
  - tests/cases/threads/TC-021_since-keeps-older-calls-out-of-a-thread.md
auto: [tests/auto/specs/S-26.spec.mjs]
---

# S-26: Ten threads in every stage: membership, people and clients

**Story:** Sam keeps ten threads, one in every stage from Lead to Cancelled, for five clients and a partner. He looks at the list (outside clients first, his own company, no client, the closed ones folded), at why each call is in its thread (series, the client's people, subject, added by hand), at a call that two threads hold, at suggestions that never assign a call, at calls taken out by hand, at the rules and their counts, at the counterparts, at what Found for you offers and at closed threads that stop taking calls. Then at the people: the map and its quadrants, the list with the people seen only in calls, who is new, a client's People tab, one person's panel and the side of each person. Then at the clients: an account against an organisation, names and domains, About and logo, a client with only closed threads, one with none, his own company, the Overview cards, the Calls and Tasks tabs. Last, he moves Acme rollout's Since date back so an older call comes in.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Standard data, built right before the run | `serve.sh up <dir> <port> --set standard` | all |
| The fold "Closed · 3" open in the Threads list (remembered in the browser) | stage 1, step 4 | 18 (reads Acme hackathon in it), 19 (Tyrell bid in it); folded again in stage 21 |
| Acme renewal's Calls tab | stage 4, step 4 | 5 |
| Acme rollout's Profile | stage 5, step 3 (its last thread) | 6 |
| The Threads list | stage 7, step 4 | 8 |
| Globex pilot's People on Map | stage 10, step 4 | 11 |
| The People view on List, Acme's client People tab | stage 12, step 5 | 13 |
| Acme rollout's People on List (the panel closed or not) | stage 14 | 15 |
| The Threads list | stage 18, step 6 | 19 |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `threads/TC-018_list-on-the-full-set-groups-stages-and-closed-fold.md` | — | Load `{{base_url}}/` (the cookie); if the browser was used before, clear `localStorage` keys `echo.threads.folded` and `echo.people.view`. **Skip its teardown**: "Closed · 3" stays open for stages 18 and 19 (stage 21 folds it) |
| 2 | `threads/TC-019_calls-tab-says-why-each-call-is-in.md` | — | — |
| 3 | `threads/TC-020_one-call-in-two-threads-with-its-own-reason-in-each.md` | — | — |
| 4 | `threads/TC-024_a-suggestion-never-assigns-a-call.md` | — | In step 4 open Acme rollout's tab first and Acme renewal's last |
| 5 | `threads/TC-022_taken-out-calls-stay-out-and-are-counted.md` | — | On stage 4's view: in step 1 start with Acme renewal (its Calls tab is shown: skip that navigation). In step 3 do the threads in this order: Acme hackathon, Legacy migration, Globex pilot, Acme renewal, Acme rollout last |
| 6 | `threads/TC-026_profile-rules-and-match-counts-on-ten-threads.md` | — | On stage 5's view: skip step 1's navigation (Acme rollout's Profile is shown) |
| 7 | `threads/TC-027_counterparts-none-one-and-several.md` | — | — |
| 8 | `threads/TC-025_found-for-you-six-candidates-and-their-client-choices.md` | — | On stage 7's view: in step 1 skip the navigation (the Threads list is shown); click "Found for you" |
| 9 | `threads/TC-023_closed-thread-keeps-its-calls-and-takes-no-new-ones.md` | — | — |
| 10 | `people/TC-009_map-places-people-and-organisations-by-influence-and-interest.md` | — | — |
| 11 | `people/TC-010_list-splits-people-in-the-thread-from-those-seen-in-calls.md` | — | On stage 10's view: in step 1 skip the navigation (Globex pilot's People are shown on Map); press "List" |
| 12 | `people/TC-012_new-tag-on-people-first-seen-within-two-weeks.md` | — | **Skip its teardown**: the People view stays on List for stage 13 |
| 13 | `people/TC-013_client-people-tab-threads-column-and-the-user-s-company.md` | — | On stage 12's view: skip step 1's navigation and its "List" press (Acme's People tab is shown on List). **Skip its teardown** (stage 14 presses List) |
| 14 | `people/TC-014_person-panel-place-sources-and-facts.md` | — | **Skip its teardown** (stage 15 reads the same list) |
| 15 | `people/TC-011_list-side-your-company-an-organisation-a-domain-or-outside.md` | — | On stage 14's view: skip step 1 (Acme rollout's People are on List) |
| 16 | `clients/TC-010_an-account-is-a-client-an-organisation-is-not.md` | — | — |
| 17 | `clients/TC-011_profile-other-names-and-domains-of-five-clients.md` | — | — |
| 18 | `clients/TC-012_about-and-logo-present-and-absent.md` | — | — (step 6: Acme hackathon is in the open fold "Closed · 3") |
| 19 | `clients/TC-013_a-client-with-only-closed-threads.md` | — | On stage 18's view: skip step 1's navigation (the Threads list is shown) |
| 20 | `clients/TC-014_a-client-with-no-threads.md` | — | — |
| 21 | `clients/TC-017_the-user-s-company-with-a-thread.md` | — | In step 1, after the navigation, click "Closed · 3" to fold it (stage 1's teardown, kept until here) |
| 22 | `clients/TC-015_overview-cards-of-a-client-with-open-and-closed-threads.md` | — | — |
| 23 | `clients/TC-016_calls-and-tasks-tabs-of-a-client-with-three-threads.md` | — | — |
| 24 | `threads/TC-021_since-keeps-older-calls-out-of-a-thread.md` | — | **Skip its teardown** (the last stage: the Since date stays moved) |

## Notes
- A stage that continues an earlier stage's view ("On stage N's view") takes one snapshot first; if the state is not there (the earlier stage failed before it), it does the skipped steps after all.
- The only stage that changes data is the last one (`threads/TC-021` moves Acme rollout's Since date); it is last so the others read the data as built, and its teardown is skipped.
- The browser remembers the folds and the People view: stage 1 opens "Closed · 3" and stage 21 folds it again; stages 12 to 14 leave the People view on List for the next one, and stage 15 ends on Map, as the scenario found it; stage 21 presses List for its step 5 and Map again in its teardown.
- Each thread is visited once per stage: `threads/TC-019` and `TC-026` go thread by thread, and the scenario picks the order of the threads so that a stage starts where the one before it ended.
- Never pressed: Accept or Reject in Found for you, "Remove this client…", "Edit", Unlink, Merge, Add (they change data for good; other cases cover them on their own).
