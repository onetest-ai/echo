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
- Two more there, answering `--version` with a bare version (a wrapper passing on its Claude Code's): `printf '#!/bin/sh\necho 2.1.400\n' > $W/acme-bare`, `printf '#!/bin/sh\necho 2.1.200\n' > $W/acme-old`, `chmod +x` both
- `{{base_url}}/` was loaded just before the first window step

**Changes:** `<config dir>/ai.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; in the line under the profiles click "Add a wrapper…" | A row "Add a wrapper": "A program you installed that starts Claude Code its own way — a name on your PATH or its full path. Echo starts it in place of Claude Code, on your own Claude Code setup. It sees and can change everything Echo sends to Claude, call transcripts included."; the fields "Program" and "Name", "Check", "Add profile" (disabled), "Cancel", and nothing under them — not what an earlier check, add or update said (after settings/TC-032: no "It now starts …") |
| 2 | Type `/bin/ls` in "Program"; click "Check"; wait 3 s | "/bin/ls doesn't answer --version as Claude Code does"; "Add profile" stays disabled |
| 2a | Type `acme-bare` in "Program"; click "Check"; wait 3 s. Then type `acme-old`; click "Check"; wait 3 s | First "Found <$W>/acme-bare — 2.1.400", "Add profile" enabled; then "Found <$W>/acme-old — 2.1.200. Claude Code 2.1.200 is older than Echo asks for: Echo's guard can't stop a tool call when the guard itself fails, and auto mode may start with more permissions when it isn't available — update it to 2.1.323 or later", "Add profile" enabled (an older Claude Code can be added; nothing is added until it is clicked) |
| 3 | Type `acme-claude` in "Program" and `Acme by hand` in "Name"; click "Check"; wait 3 s | "Found <$W>/acme-claude — 2.1.400 (Claude Code)"; "Add profile" enabled |
| 4 | Type `acme-claude2` in "Program" (no Check); click "Add profile" | Nothing is added: "The program changed since its check — Check it again."; "Add profile" disabled. Type `acme-claude` again and click "Check"; wait 3 s: "Found …/acme-claude …" again |
| 5 | Click "Add profile"; wait 3 s | The form is gone; a notice "Added Acme by hand — it starts <$W>/acme-claude"; a row "Acme by hand" tagged "Wrapper": "Starts <$W>/acme-claude in place of Claude Code. It sees and can change everything Echo sends to Claude, call transcripts included." |
| 6 | `$E ai status` | A profile `acme-claude`, kind `wrapper`, `"name": "Acme by hand"`, `"offered_by": null` |

## Expected Final State
A wrapper profile "Acme by hand".

## Teardown
- `$E ai remove acme-claude`; `rm $W/acme-bare $W/acme-old`.
