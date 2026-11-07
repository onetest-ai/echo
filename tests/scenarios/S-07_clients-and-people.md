---
id: S-07
title: Clients and people: a client's page and tabs, a new thread and domains, a thread's people, a person's details
run: agent
data: small
cases:
  - tests/cases/clients/TC-008_own-company-is-your-company.md
  - tests/cases/clients/TC-001_open-a-client.md
  - tests/cases/clients/TC-002_overview-about-and-thread-card.md
  - tests/cases/clients/TC-003_calls-and-tasks-tabs.md
  - tests/cases/clients/TC-004_profile-other-names-and-domains.md
  - tests/cases/clients/TC-005_new-thread-for-a-client.md
  - tests/cases/clients/TC-007_client-with-threads-cannot-be-removed.md
  - tests/cases/clients/TC-006_remove-a-client-without-threads.md
  - tests/cases/clients/TC-009_organisations-that-are-not-clients-cli.md
  - tests/cases/people/TC-001_empty-thread-offers-people-from-calls.md
  - tests/cases/people/TC-003_list-shows-side-org-and-add.md
  - tests/cases/people/TC-002_plus-person-adds-someone.md
  - tests/cases/people/TC-004_map-drag-to-a-quadrant.md
  - tests/cases/people/TC-005_person-panel-edit-in-place.md
  - tests/cases/people/TC-006_suggestions-yes-no-change.md
  - tests/cases/people/TC-007_same-person-merge-and-split.md
  - tests/cases/people/TC-008_name-an-unknown-organisation.md
---

# S-07: Clients and people: a client's page and tabs, a new thread and domains, a thread's people, a person's details

**Story:** Sam checks that Initech is his own company, opens Acme and Globex pages, edits their names and domains, makes a thread for a client, tries to remove clients, then builds a thread's people: seen people, a person added, the map, a person's details, suggestions and merging.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `clients/TC-008_own-company-is-your-company.md` | — | — |
| 2 | `clients/TC-001_open-a-client.md` | — | — |
| 3 | `clients/TC-002_overview-about-and-thread-card.md` | — | — |
| 4 | `clients/TC-003_calls-and-tasks-tabs.md` | — | — |
| 5 | `clients/TC-004_profile-other-names-and-domains.md` | — | — |
| 6 | `clients/TC-005_new-thread-for-a-client.md` | — | — |
| 7 | `clients/TC-007_client-with-threads-cannot-be-removed.md` | — | — |
| 8 | `clients/TC-006_remove-a-client-without-threads.md` | — | — |
| 9 | `clients/TC-009_organisations-that-are-not-clients-cli.md` | — | — |
| 10 | `people/TC-001_empty-thread-offers-people-from-calls.md` | — | — |
| 11 | `people/TC-003_list-shows-side-org-and-add.md` | — | — |
| 12 | `people/TC-002_plus-person-adds-someone.md` | — | — |
| 13 | `people/TC-004_map-drag-to-a-quadrant.md` | — | — |
| 14 | `people/TC-005_person-panel-edit-in-place.md` | — | — |
| 15 | `people/TC-006_suggestions-yes-no-change.md` | — | — |
| 16 | `people/TC-007_same-person-merge-and-split.md` | — | — |
| 17 | `people/TC-008_name-an-unknown-organisation.md` | — | — |

## Notes
- Every people stage makes its own thread `people-lab` and deletes it, so no stage depends on another.
- Stage 17 creates organisation `contoso` and removes it; stage 9 uses `contoso-partners`.
- Stage 6 adds a thread under Globex and deletes it; stage 7 reads Acme, which still has Acme rollout.
- Stage 15 writes its suggestions with `/dev/stdin` in its preconditions.
