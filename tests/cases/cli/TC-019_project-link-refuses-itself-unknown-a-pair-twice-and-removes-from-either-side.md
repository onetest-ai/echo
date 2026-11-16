---
id: TC-019
title: echoctl project link refuses the thread itself, an unknown thread, a pair already linked from the other side and a loop; a new parent replaces the old; --remove works from either side; a deleted thread drops out
priority: medium
type: functional
requirements: [F-THR-22, F-THR-24]
module: cli
run: agent
platform: any
tags: [cli, threads, links]
---

# TC-019: project link refuses what it can't link, replaces a parent, removes from either side

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh` (small set); `. <dir>/env` sourced for every `echoctl` command
- `target/debug/echoctl project create "Acme platform" --client acme` and `target/debug/echoctl project create "Acme program"` run first (ids `acme-platform`, `acme-program`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl project link acme-rollout --to acme-rollout --kind related` | Exit 1, "a thread can't be linked to itself" |
| 2 | `target/debug/echoctl project link acme-rollout --to nope --kind related` | Exit 1, `no thread "nope"` |
| 3 | `target/debug/echoctl project link acme-rollout --to acme-platform` | Exit 2 (usage): `--kind` is needed without `--remove` |
| 4 | `target/debug/echoctl project link acme-rollout --to acme-platform --kind part-of`, then `target/debug/echoctl project link acme-platform --to acme-rollout --kind related` | The first exits 0; the second exits 1: "Acme rollout already links to this thread: change it there (`project link acme-rollout --to acme-platform --remove`)" |
| 5 | `target/debug/echoctl project link acme-rollout --to acme-program --kind part-of` | Exit 0; `links` has one `part_of`, `acme-program` (Acme platform is no longer its parent) |
| 6 | `target/debug/echoctl project link acme-program --to acme-platform --kind part-of`, then `target/debug/echoctl project link acme-platform --to acme-rollout --kind part-of` | The first exits 0; the second exits 1: "Acme rollout is part of this thread already (a loop)" (Acme rollout → Acme program → Acme platform) |
| 7 | `target/debug/echoctl project link acme-program --to acme-rollout --remove` (the parent's side) | Exit 0; `project show acme-rollout` has `links` `[]` |
| 8 | `target/debug/echoctl project link acme-platform --to globex-pilot --remove` | Exit 1, "acme-platform and globex-pilot aren't linked" |
| 9 | `target/debug/echoctl project link globex-pilot --to acme-platform --kind related`, then `target/debug/echoctl project delete acme-platform`, then `target/debug/echoctl project show globex-pilot` | `links` `[]`: a thread in the trash drops out |

## Expected Final State
No links left; Acme platform in the trash; Acme program exists.

## Teardown
- None needed (fresh data per run)

## Changes
- Makes two threads, deletes one.

## Notes
- A run Echo started can't link threads: `project link` is not a read (`projects can't be changed or listed from a run Echo started`).
