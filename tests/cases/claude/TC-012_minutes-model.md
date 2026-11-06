---
id: TC-012
title: Each AI profile's minutes model is used, and a wrong one is explained
priority: medium
type: functional
requirements: [F-AI-01, F-SET-12, F-AI-08]
module: claude
run: person
platform: macos
tags: [minutes, ai-profile]
external_id: TC-CLA-011
---

# TC-012: Each AI profile's minutes model is used, and a wrong one is explained

**Module:** Claude Code | **Priority:** Medium | **Type:** Functional

## Preconditions
- Claude Code installed and signed in; Echo running
- Two AI profiles

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Menu → AI for calls → "Minutes model for <profile>" → `haiku`; write minutes for a call of it | The log's first lines show haiku; the Minutes tab named the same model |
| 2 | Set it to `no-such-model`; write again | "Claude Code can't use the model…" and where to change it (the menu and `echoctl ai set`) |
| 3 | Set the built-in profile's model to `default` | The run uses Claude Code's own setting |
| 4 | Run `echoctl ai set <other profile> --notes-model opus` | That profile shows opus; the other is unchanged |

## Expected Final State
The minutes model is per profile, shown where it is used, and a bad one is explained.
