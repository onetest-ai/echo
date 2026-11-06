---
id: S-26
title: Ten threads in every stage: membership, people and clients
run: agent
data: standard
cases:
  - tests/cases/threads/TC-018_list-on-the-full-set-groups-stages-and-closed-fold.md
  - tests/cases/threads/TC-019_calls-tab-says-why-each-call-is-in.md
  - tests/cases/threads/TC-020_one-call-in-two-threads-with-its-own-reason-in-each.md
  - tests/cases/threads/TC-022_taken-out-calls-stay-out-and-are-counted.md
  - tests/cases/threads/TC-023_closed-thread-keeps-its-calls-and-takes-no-new-ones.md
  - tests/cases/threads/TC-024_a-suggestion-never-assigns-a-call.md
  - tests/cases/threads/TC-026_profile-rules-and-match-counts-on-ten-threads.md
  - tests/cases/threads/TC-027_counterparts-none-one-and-several.md
  - tests/cases/threads/TC-025_found-for-you-six-candidates-and-their-client-choices.md
  - tests/cases/people/TC-009_map-places-people-and-organisations-by-influence-and-interest.md
  - tests/cases/people/TC-010_list-splits-people-in-the-thread-from-those-seen-in-calls.md
  - tests/cases/people/TC-011_list-side-your-company-an-organisation-a-domain-or-outside.md
  - tests/cases/people/TC-012_new-tag-on-people-first-seen-within-two-weeks.md
  - tests/cases/people/TC-013_client-people-tab-threads-column-and-the-user-s-company.md
  - tests/cases/people/TC-014_person-panel-place-sources-and-facts.md
  - tests/cases/clients/TC-010_an-account-is-a-client-an-organisation-is-not.md
  - tests/cases/clients/TC-011_profile-other-names-and-domains-of-five-clients.md
  - tests/cases/clients/TC-012_about-and-logo-present-and-absent.md
  - tests/cases/clients/TC-013_a-client-with-only-closed-threads.md
  - tests/cases/clients/TC-014_a-client-with-no-threads.md
  - tests/cases/clients/TC-015_overview-cards-of-a-client-with-open-and-closed-threads.md
  - tests/cases/clients/TC-016_calls-and-tasks-tabs-of-a-client-with-three-threads.md
  - tests/cases/clients/TC-017_the-user-s-company-with-a-thread.md
  - tests/cases/threads/TC-021_since-keeps-older-calls-out-of-a-thread.md
---

# S-26: Ten threads in every stage: membership, people and clients

**Story:** Sam keeps ten threads, one in every stage from Lead to Cancelled, for five clients and a partner. He looks at the list (outside clients first, his own company, no client, the closed ones folded), at why each call is in its thread (series, the client's people, subject, added by hand), at a call that two threads hold, at calls taken out by hand, at closed threads that stop taking calls, at suggestions that never assign a call, at the rules and their counts, at the counterparts and at what Found for you offers. Then at the people: the map and its quadrants, the list with the people seen only in calls, the side of each person, who is new, a client's People tab and one person's panel. Then at the clients: an account against an organisation, names and domains, About and logo, a client with only closed threads, one with none, the Overview cards, the Calls and Tasks tabs, and his own company. Last, he moves Acme rollout's Since date back so an older call comes in, and restores it.

**Data:** `standard` (`tests/fixtures/serve.sh up <dir> <port> --set standard`). Fresh data per run.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `threads/TC-018_list-on-the-full-set-groups-stages-and-closed-fold.md` | — | Reload `{{base_url}}/` (the cookie); if the browser was used before, clear `localStorage` keys `echo.threads.folded` and `echo.people.view` |
| 2 | `threads/TC-019_calls-tab-says-why-each-call-is-in.md` | — | — |
| 3 | `threads/TC-020_one-call-in-two-threads-with-its-own-reason-in-each.md` | — | — |
| 4 | `threads/TC-022_taken-out-calls-stay-out-and-are-counted.md` | — | — |
| 5 | `threads/TC-023_closed-thread-keeps-its-calls-and-takes-no-new-ones.md` | — | — |
| 6 | `threads/TC-024_a-suggestion-never-assigns-a-call.md` | — | — |
| 7 | `threads/TC-026_profile-rules-and-match-counts-on-ten-threads.md` | — | — |
| 8 | `threads/TC-027_counterparts-none-one-and-several.md` | — | — |
| 9 | `threads/TC-025_found-for-you-six-candidates-and-their-client-choices.md` | — | — |
| 10 | `people/TC-009_map-places-people-and-organisations-by-influence-and-interest.md` | — | — |
| 11 | `people/TC-010_list-splits-people-in-the-thread-from-those-seen-in-calls.md` | — | — |
| 12 | `people/TC-011_list-side-your-company-an-organisation-a-domain-or-outside.md` | — | — |
| 13 | `people/TC-012_new-tag-on-people-first-seen-within-two-weeks.md` | — | — |
| 14 | `people/TC-013_client-people-tab-threads-column-and-the-user-s-company.md` | — | — |
| 15 | `people/TC-014_person-panel-place-sources-and-facts.md` | — | — |
| 16 | `clients/TC-010_an-account-is-a-client-an-organisation-is-not.md` | — | — |
| 17 | `clients/TC-011_profile-other-names-and-domains-of-five-clients.md` | — | — |
| 18 | `clients/TC-012_about-and-logo-present-and-absent.md` | — | — |
| 19 | `clients/TC-013_a-client-with-only-closed-threads.md` | — | — |
| 20 | `clients/TC-014_a-client-with-no-threads.md` | — | — |
| 21 | `clients/TC-015_overview-cards-of-a-client-with-open-and-closed-threads.md` | — | — |
| 22 | `clients/TC-016_calls-and-tasks-tabs-of-a-client-with-three-threads.md` | — | — |
| 23 | `clients/TC-017_the-user-s-company-with-a-thread.md` | — | — |
| 24 | `threads/TC-021_since-keeps-older-calls-out-of-a-thread.md` | — | — |

## Notes
- The only stage that changes data is the last one (`threads/TC-021` moves Acme rollout's Since date and puts it back); it is last so the others read the data as built.
- Stages that fold a group or switch the People view put it back in their teardown.
- Never pressed: Accept or Reject in Found for you, "Remove this client…", "Edit", Unlink, Merge, Add (they change data for good; other cases cover them on their own).
