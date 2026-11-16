---
id: TC-033
title: Add a wrapper… checks the program typed in — where the shell finds it and its version — before it can be added
priority: high
type: functional
requirements: [F-AI-01, F-AI-14]
module: settings
run: agent
platform: macos
data: standard
tags: [ai, profiles, wrapper]
---

# TC-033: Add a wrapper… checks the program before it can be added

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- As settings/TC-032: `$W/acme-claude` on the PATH the served window sees (served with `PATH="$W:$PATH"`)
- `{{base_url}}/` was loaded just before the first window step

**Changes:** `<config dir>/ai.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; in the line under the profiles click "Add a wrapper…" | A row "Add a wrapper": "A program you installed that starts Claude Code its own way — a name on your PATH or its full path. Echo starts it in place of Claude Code, on your own Claude Code setup. It sees and can change everything Echo sends to Claude, call transcripts included."; the fields "Program" and "Name", "Check", "Add profile" (disabled), "Cancel", and nothing under them — not what an earlier check, add or update said (after settings/TC-032: no "It now starts …") |
| 2 | Type `/bin/ls` in "Program"; click "Check"; wait 3 s | "/bin/ls doesn't answer --version as Claude Code does"; "Add profile" stays disabled |
| 3 | Type `acme-claude` in "Program" and `Acme by hand` in "Name"; click "Check"; wait 3 s | "Found <$W>/acme-claude — 2.1.400 (Claude Code)"; "Add profile" enabled |
| 4 | Click "Add profile"; wait 3 s | The form is gone; a row "Acme by hand" tagged "Wrapper": "Starts <$W>/acme-claude in place of Claude Code. It sees and can change everything Echo sends to Claude, call transcripts included." |
| 5 | `$E ai status` | A profile `acme-claude`, kind `wrapper`, `"name": "Acme by hand"`, `"offered_by": null` |

## Expected Final State
A wrapper profile "Acme by hand".

## Teardown
- `$E ai remove acme-claude`.
