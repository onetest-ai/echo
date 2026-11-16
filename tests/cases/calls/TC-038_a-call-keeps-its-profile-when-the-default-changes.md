---
id: TC-038
title: A call keeps the AI profile it was pinned to when the default changes; a call with none follows the default
priority: critical
type: functional
requirements: [F-AI-03]
module: calls
run: agent
platform: any
data: standard
ai: local
tags: [calls, minutes, ai-profiles, fake-claude]
---

# TC-038: A call keeps the AI profile it was pinned to when the default changes; a call with none follows the default

**Module:** Calls | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- The default AI profile is the local one (`ollama`). `<profile>` and `<model>` below are its `name` and `notes_model` in `target/debug/echoctl ai status`, and `<where>` is "on this computer" for a model on this Mac or "at <host>" for one on the network (for example "<profile>", `qwen3.8:27b-mlx`, "on this computer"). A recorded call is pinned to a profile when its recording starts (`meta.json` → `ai_profile`); on this data only Vendor call with Vandelay is (to `out`). Choosing another profile in the Minutes tab's Model select and pressing Write minutes / Try again pins it too: this case pins one call that way to `work`, whose run the fake `claude` refuses at once — no model run
- The line under a Minutes box, "Claude Code writes them with …", and the Model select (`select[data-notesprofile]`, "Model for these minutes") name the call's profile
- **In S-30** `<profile>`, `<model>` and `<where>` are read once, in the scenario's prelude (`echoctl ai status` and one `ls` of the calls), and every stage uses them. On its own, read them as above

## Test Data

| Call | `ai_profile` before | Default `ollama` | After the pin, default `system` |
|---|---|---|---|
| Umbrella proposal walkthrough (`umbrella-proposal-walkthrough`, "No minutes for this call") | none | "…with <profile> (<model>), <where>." | pinned to `work`: "…with Claude · work (sonnet), with your Claude account (sam@initech.example · initech)." |
| Vendor call with Vandelay (`vendor-call-with-vandelay`, needs sign-in) | `out` | "…with Claude · out (sonnet), with your Claude account." | the same |
| Acme security review (`acme-security-review`, failed) | none | "…with <profile>…" | "…with sonnet, as it is set up (…)", select "Claude Code (as configured)" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, wait for "Acme weekly sync", click the row `[data-call$="umbrella-proposal-walkthrough"]`, tab "Minutes" | "No minutes for this call"; the line "Claude Code writes them with <profile> (<model>), <where>."; the select "Model for these minutes" shows "<profile>"; a "Write minutes" button |
| 2 | Run `grep -c ai_profile $ECHOCTL_HOME/calls/*-umbrella-proposal-walkthrough/meta.json` | `0` (not pinned) |
| 3 | In "Model for these minutes" select "Claude · work — sam@initech.example · initech" (`browser_select_option`), then click "Write minutes"; wait up to 15 seconds for "test data: Claude Code is a fake here" | The box becomes "Couldn't write the minutes" with "test data: Claude Code is a fake here"; the line reads "Claude Code writes them with Claude · work (sonnet), with your Claude account (sam@initech.example · initech)." |
| 4 | Run `grep '"ai_profile"' $ECHOCTL_HOME/calls/*-umbrella-proposal-walkthrough/meta.json` | `"ai_profile": "work",` |
| 5 | Run `target/debug/echoctl ai default system` | `new calls use Claude Code (as configured)` |
| 6 | Reload `{{base_url}}/`, open Umbrella proposal walkthrough's Minutes tab | Still "…with Claude · work (sonnet)…"; the select shows "Claude · work — sam@initech.example · initech" |
| 7 | Click the row `[data-call$="vendor-call-with-vandelay"]` (Minutes tab) | "Claude Code needs you to sign in", "Sign in to Claude (out) first", the line "Claude Code writes them with Claude · out (sonnet), with your Claude account."; the select shows "Claude · out — not signed in" |
| 8 | Click the row `[data-call$="acme-security-review"]` (Minutes tab) | The line now reads "Claude Code writes them with sonnet, as it is set up (menu → AI for calls → Minutes model, while it is the profile new calls use)."; the select shows "Claude Code (as configured)" — a call without a pin follows the default |
| 9 | Run `target/debug/echoctl ai default ollama`, reload, open Umbrella proposal walkthrough's Minutes tab | `new calls use <profile>`; the call still says "…with Claude · work (sonnet)…" |

## Expected Final State
The default is back on <profile>. Umbrella proposal walkthrough stays pinned to Claude · work (its last run failed against the fake); Vendor call with Vandelay stays on Claude · out.

## Teardown
- The default is restored in step 9; the pin of Umbrella proposal walkthrough stays (later stages of a scenario must not run minutes on it)

## Notes
- Verified 2026-11-07 on a served window: every text above.
- Only a recording (or a profile chosen for a run) pins a call; the made-up calls without `ai_profile` stand for calls recorded before profiles existed, which follow the default (`ai::call_profile_id`).
