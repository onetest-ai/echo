---
id: TC-030
title: A thread linked as part of another and related to a third shows "Part of … →" and "Related:" in its header and Linked threads on its Brief; the parent lists it as a part
priority: high
type: functional
requirements: [F-THR-22, F-THR-23, F-THR-24]
module: threads
run: agent
platform: any
tags: [threads, links, header, brief]
---

# TC-030: Part of and related links show in the header and on the Brief, on both sides

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` (small set); window served at `{{base_url}}` and `{{base_url}}/` loaded first (the cookie); `. <dir>/env` sourced for every `echoctl` command

## Test Data

| Thread | Before | After |
|-------|-------|-------|
| Acme platform (`acme-platform`, client Acme) | doesn't exist | made in step 1; Acme rollout is part of it |
| Acme rollout (`acme-rollout`) | no links | part of Acme platform, related to Globex pilot |
| Globex pilot (`globex-pilot`) | no links | related to Acme rollout (kept on Globex pilot's side) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl project create "Acme platform" --client acme` | JSON with `id` "acme-platform" |
| 2 | Run `target/debug/echoctl project link acme-rollout --to acme-platform --kind part-of` | Exit 0; JSON of Acme rollout whose `links` is one entry: `id` "acme-platform", `name` "Acme platform", `kind` "part_of", `stage` "active", `work.open_count` 0 |
| 3 | Run `target/debug/echoctl project link globex-pilot --to acme-rollout --kind related` | Exit 0; Globex pilot's `links`: `acme-rollout`, `kind` "related" |
| 4 | Run `target/debug/echoctl project show acme-rollout` and read `links` | Two entries, in this order: `acme-platform` `part_of`, then `globex-pilot` `related` with `work.open_count` 1 and `work.open[0].title` "Draft the Globex pilot plan" |
| 5 | Navigate to `{{base_url}}/#project/acme-rollout/brief` | Under the facts line: "Part of Acme platform →" and "Related: Globex pilot" (links) |
| 6 | Read the Brief | Between "What it is" and "Latest calls" a section "Linked threads" (list "Linked threads") with two rows: "Is part of" Acme platform, tag Active, "0 open"; "Related" Globex pilot, tag Active, "1 open", and under it "Task Draft the Globex pilot plan · Sam Rivera" |
| 7 | Click "Acme platform →" in the header | The URL is `#project/acme-platform/brief`; heading "Acme platform"; no "Part of" or "Related:" line in its header |
| 8 | Read Acme platform's Brief | "Linked threads" has one row: "Part of this" Acme rollout, tag Active, "2 open", with "Task Send Acme the SSO checklist · Sam Rivera" and "Task Answer the security questionnaire · Omar Haddad" |
| 9 | Navigate to `{{base_url}}/#project/globex-pilot/brief` | Header: "Related: Acme rollout" (no "Part of"); Linked threads: "Related" Acme rollout, "2 open" |

## Expected Final State
Acme platform exists; Acme rollout is part of it and related to Globex pilot; each of the three shows its links.

## Teardown
- None needed (fresh data per run)

## Changes
- Makes the thread Acme platform and two links.

## Notes
- A link is kept on one side only (the part's profile; for related the thread it was made from) and read from both: Acme platform's profile has no `links`.
