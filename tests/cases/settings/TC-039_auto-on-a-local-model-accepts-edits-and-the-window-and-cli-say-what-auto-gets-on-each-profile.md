---
id: TC-039
title: "Claude's permissions: Auto on a local model's profile is edits accepted, not auto mode, and the card's last line and `echoctl config` say what Auto gets on each profile"
priority: high
type: functional
requirements: [F-SET-24]
module: settings
run: agent
platform: any
data: standard
tags: [settings, claude, permissions, ai-profiles, cli]
---

# TC-039: Auto on a local model accepts edits, and the window and the CLI say what Auto gets on each profile

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Fresh standard test data (`tests/fixtures/serve.sh up <dir> <port> --set standard`: profiles "Claude Code (as configured)", "Claude · work", "Claude · other", "Claude · out" and the local model "Qwen · this computer"), window served at `{{base_url}}`, `{{base_url}}/` loaded first (the cookie)
- CLI steps in Bash after `. <dir>/env`; `E=target/debug/echoctl`, built right before the run; abort if `$ECHOCTL_HOME` isn't inside `<dir>`

**Changes:** `config.json` (step 2 writes Yolo's setting as Auto, its default)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/ai`; read the last line of the card "Claude's permissions" | It starts "Auto on your profiles:" and names each of the five profiles; "Qwen · this computer" — "edits accepted: this profile's model has no auto mode" (auto mode needs a recent Claude model) |
| 2 | `$E config --yolo-permissions auto 2>&1 >/dev/null` | One `Auto on <profile>: …` line per profile, the same as the card's: the four Claude profiles "auto mode: Claude Code decides each action", "Qwen · this computer" "edits accepted: this profile's model has no auto mode" |

## Expected Final State
Yolo's setting written as Auto (its default); nothing else changed.

## Teardown
- On its own: `tests/fixtures/serve.sh down <dir>`

## Notes
- What the line says for a Claude account or Claude Code on the small set: `settings/TC-014` step 4.
- Which models have auto mode (Claude's names and aliases, recent ids, Bedrock and Vertex): the Rust test `folder_args::auto_is_claude_codes_auto_mode_only_where_the_profiles_model_has_it_else_edits_accepted_or_read_only`. What Yolo on Auto does on a local model (its send left as a draft): `tasks/TC-031`.
