---
id: TC-009
title: The map puts each person in a quadrant by influence and interest, and the rest in "Not placed yet"
priority: medium
type: functional
requirements: [F-PPL-02, F-PPL-01]
module: people
run: agent
platform: any
tags: [people, map, quadrants, pairwise]
auto: [tests/auto/specs/S-26.spec.mjs]
---

# TC-009: The map puts each person in a quadrant by influence and interest, and the rest in "Not placed yet"

**Module:** People | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- The People view is Map (`localStorage` key `echo.people.view` unset or "map"); no person has been dragged

## Test Data

| Thread | Quadrant | Chips: name (line under it) |
|---|---|---|
| Acme rollout | Keep satisfied | Omar Haddad (security), Lena Vogel (your company) |
| Acme rollout | Manage closely | Dana Lee (delivery lead), Alex Chen (architect) |
| Acme rollout | Monitor | Ola Svensson (Acme), Tom Becker (migration lead) |
| Acme rollout | Keep informed | Rita Gomez (Acme) |
| Acme rollout | Not placed yet | Ivan Petrov (Acme, new), Yuki Tanaka (Acme, new), Vandelay (organisation), Northwind Analysts (organisation) |
| Globex pilot | Keep satisfied | - |
| Globex pilot | Manage closely | Kim Park (sponsor, new) |
| Globex pilot | Monitor | - |
| Globex pilot | Keep informed | Alex Chen (Solution architect) |
| Globex pilot | Not placed yet | Lena Vogel (your company), Vandelay (organisation) |

Quadrant titles and subtitles: "Keep satisfied" (high influence · low interest), "Manage closely" (high influence · high interest), "Monitor" (low influence · low interest), "Keep informed" (low influence · high interest). Levels in the data are 1 (low) and 3 (high); a person without both levels waits in "Not placed yet".

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/people`, wait for the text "Influence", and press "Map" in the segmented control if it is not pressed | The tab reads "People" with the number 9; the legend shows champion, supporter, neutral, sceptic, blocker and the button "+ Person" |
| 2 | Read the four quadrants (`section.pp-q`, named by `aria-label`) and the strip "Not placed yet" | Each holds the chips of the Acme rollout rows of the table and no others; the avatar of a chip has the stance as a class (Dana Lee `champion`, Omar Haddad `sceptic`, Rita Gomez `supporter`, Ola Svensson `blocker`, Ivan Petrov `neutral`) |
| 3 | Read the line under the chips | "Drag a person to another quadrant. 4 more were in the calls and aren't in this thread yet — List." |
| 4 | Navigate to `{{base_url}}/#project/globex-pilot/people` and read the quadrants and the strip | The Globex pilot rows of the table: Kim Park in "Manage closely" (sponsor), Alex Chen in "Keep informed" (his place here is 1 / 3; in Acme rollout he is high / high), Lena Vogel and the organisation Vandelay in "Not placed yet" |
| 5 | Read the line under the chips | "Drag a person to another quadrant. 4 more were in the calls and aren't in this thread yet — List." |

## Expected Final State
Both maps match the table; nobody was moved.

## Teardown
- None: the case only reads

## Notes
- Covers person.in_thread (placed or seen only) by person.levels (high / low, none, one level only) by side (yours: Alex Chen, Lena Vogel, Tom Becker; theirs; an organisation).
- A person is placed in the thread they are viewed in: the same Alex Chen is in another quadrant in each thread.
- Does not drag: `TC-004` does.
