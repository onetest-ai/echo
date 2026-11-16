---
id: TC-012
title: A person first seen in the last 14 days carries the tag "new", and it depends on the scope viewed
priority: low
type: functional
requirements: [F-PPL-04]
module: people
run: agent
platform: any
tags: [people, recency, new]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-012: A person first seen in the last 14 days carries the tag "new", and it depends on the scope viewed

**Module:** People | **Priority:** Low | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No person has been added or merged since the data was built

## Test Data

| Thread | Person | First seen | Tag |
|---|---|---|---|
| Acme rollout | Ivan Petrov | first seen yesterday, after the thread's first call | new |
| Acme rollout | Yuki Tanaka | first seen today | new |
| Globex pilot | Kim Park | first seen yesterday (the kickoff) | new |
| Acme rollout | Dana Lee | first seen 14 days ago, at the thread's first call | - |
| Acme rollout | Ola Svensson | never seen in the thread's calls | - |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/people`, press "Map" | The chips of Ivan Petrov and Yuki Tanaka (in "Not placed yet") carry the tag "new" (`.pp-new`); no other chip has it |
| 2 | Press "List" | The names of Ivan Petrov and Yuki Tanaka carry the tag "new"; no row has the tag "quiet" (`.pp-quiet`) |
| 3 | Navigate to `{{base_url}}/#project/globex-pilot/people` | Kim Park carries "new"; Alex Chen and Lena Vogel do not |
| 4 | Run `echoctl project people acme-rollout` and read `new` and `quiet` of Dana Lee, Ivan Petrov, Yuki Tanaka and Ola Svensson | `new` is true for Ivan Petrov and Yuki Tanaka only; `quiet` is false for all four |
| 5 | Navigate to `{{base_url}}/#client/acme/people` | In this scope Ivan Petrov and Alex Chen carry "new" and Yuki Tanaka does not (he was in the Acme hackathon call earlier) |

## Expected Final State
Nothing changed.

## Teardown
- Press "Map" in the segmented control

## Changes
- Changes the remembered People view in the browser.

## Notes
- The tag "quiet" (not seen for six weeks while the calls went on) cannot be shown by this data: the fixture lists Ola Svensson as quiet, but her only call is last year's kickoff, which Acme rollout's since date leaves out, so she was never seen in the thread and `quiet` is false.
