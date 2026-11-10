---
id: TC-027
title: The account chip in the top bar shows the default AI profile; with two it picks the default, with one it opens AI & minutes
priority: high
type: functional
requirements: [F-AI-09, F-AI-01, F-WS-07]
module: settings
run: agent
platform: any
tags: [ai, profiles, top-bar]
auto: [tests/auto/specs/S-12.spec.mjs]
---

# TC-027: The account chip in the top bar shows the default AI profile; with two it picks the default, with one it opens AI & minutes

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh`, window served at `{{base_url}}` (`tests/fixtures/serve.sh up`); only the built-in profile exists (`echoctl ai status` lists one)
- `{{base_url}}/` was loaded just before the first window step (its cookie lets clicks reach the app; a second Echo server on 127.0.0.1 replaces the cookie, so load `{{base_url}}/` again if a click has no effect)
- CLI steps run in Bash after `. <dir>/env` (`<dir>` is the run's data folder)

**Changes:** `<config dir>/ai.json` (a profile added, then key `default`)

## Test Data

| Field | Value |
|---|---|
| Account chip | `#r-top .acct`; what it reads is its label `#r-top .acct > span` (with a select, the element's own text also holds the options): a button with one profile, a `label` over `select[aria-label="AI profile"]` with two or more |
| Second profile | Local Llama (`local-llama`), a local model that is never contacted |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today` and wait for "Your day"; read the top bar's right end | The account chip reads "Claude Code (as configured)" and sits just left of the button "Inbox"; it is a button (named "AI profile: Claude Code (as configured)"), there is no `select[aria-label="AI profile"]` and no caret (`#r-top .acct svg` finds nothing) |
| 2 | Read the chip's tooltip (`title`) | It names "Claude Code (as configured)" and "as Claude Code is set up" |
| 3 | Click the chip | The URL becomes `#settings/ai` and the heading "AI & minutes" shows |
| 4 | In Bash run `echoctl ai add-local local-llama --model llama3.2 --url http://127.0.0.1:9 --name "Local Llama"` | Prints `added Local Llama (local-llama)` |
| 5 | Reload `{{base_url}}/#today` and wait for "Your day"; read the chip | It reads "Claude Code (as configured)" with a caret; `select[aria-label="AI profile"]` holds two options, "Claude Code (as configured)" (selected) and "Local Llama" |
| 6 | Read the Record pill (`#r-top .pill`) | The button "Record next call", the select "Language of the call" and the button "Memo"; `#r-top .pill select` finds exactly one select (no profile segment) |
| 7 | Select "Local Llama" in "AI profile" (`browser_select_option`) | The chip reads "Local Llama" |
| 8 | Wait 1 s, run `. <dir>/env && cat "$ECHOCTL_CONFIG_DIR/ai.json"` | The file has `"default": "local-llama"` |
| 9 | Reload `{{base_url}}/#today` and wait for "Your day" | The chip still reads "Local Llama" (its select has "Local Llama" selected) |
| 10 | Navigate to `{{base_url}}/#settings/ai`; read the card "Profiles" | "Local Llama" has the tag "Default for new calls"; "Claude Code (as configured)" has a button "Make default" |

## Expected Final State
Local Llama is the default in the chip, Settings → AI & minutes and `ai.json`, after a reload too.

## Teardown
- `echoctl ai default system` (prints `new calls use Claude Code (as configured)`), then `echoctl ai remove local-llama` (prints `removed local-llama`).

## Notes
- The profile's address is `http://127.0.0.1:9`, where nothing listens: `echoctl ai status` sends a local profile's server a request, and the Ollama on this computer (`localhost:11434`) is shared, never a test's.
- No model server is needed: the profile is only added and made the default, never used.
- A Claude profile that isn't signed in is made the default as Settings' "Make default" does it: nothing starts by itself; the sign-in note under the Record pill offers "Sign in…" (not checked here: it needs a signed-out Claude profile).
