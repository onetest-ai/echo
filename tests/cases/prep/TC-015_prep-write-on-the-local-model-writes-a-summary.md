---
id: TC-015
title: echoctl prep --write for a thread's meeting on the local model writes a prep with an In short summary, shown in the window
priority: high
type: functional
requirements: [F-PREP-05]
module: prep
run: agent
platform: macos
data: standard
ai: local
tags: [prep, cli, local-model]
---

# TC-015: echoctl prep --write for a thread's meeting on the local model writes a prep with an In short summary, shown in the window

**Module:** Prep | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built with `--ai local` right before the run (Acme renewal follow-up starts about 3 hours after the build, so it is still to come): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai local` (`data: standard`, `ai: local`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- The data's model server answering (`--ai local`). `<profile>` and `<model>` below are the default AI profile's `name` and `notes_model` in `target/debug/echoctl ai status`. Acme renewal has no AI profile of its own, so its meetings' preps are written on the default; a local profile's summary is one short request to the model server (no Claude Code). No Claude account is used
- **One model job** (step 2): seconds when the model is loaded, about a minute when the server has to load the model. Assert files, fields and the window, never the summary's wording
- The meeting's prep id is stable for the data: Acme renewal follow-up = `99067d7f0917f050` (check it in step 1)
- **In S-30** `<profile>` and `<model>` are read once, in the scenario's prelude (`echoctl ai status` and one `ls` of the calls), and every stage uses them. On its own, read them as above

## Test Data

| Meeting | Thread | Before | After |
|---|---|---|---|
| Acme renewal follow-up (Rita Gomez, today, 30 min) | Acme renewal | no prep (`prep: null`) | `<home>/prep/99067d7f0917f050/prep.json` with `summary`, `profile` "ollama", `model` "<model>", no `error` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl prep --upcoming` and find the meeting whose `subject` is "Acme renewal follow-up" | Its `id` is `99067d7f0917f050`, `threads` `[{"id": "acme-renewal", "name": "Acme renewal"}]`, `prep` null |
| 2 | Note the time; run `target/debug/echoctl prep --write 99067d7f0917f050` | Exit 0 within 3 minutes; JSON with `id` "99067d7f0917f050", `subject` "Acme renewal follow-up", `profile` "ollama", `model` "<model>", no `error` (absent or null), `written_at` now, and a non-empty `summary` (a few sentences); also `expected`, `open`, `people`, `threads` |
| 3 | Run `ls $ECHOCTL_HOME/prep/99067d7f0917f050/` and `target/debug/echoctl prep --show 99067d7f0917f050` | `prep.json`; `--show` prints the same prep (`summary` as in step 2) |
| 4 | Run `target/debug/echoctl prep --upcoming` again | Acme renewal follow-up's `prep` now has `written_at` (step 2's) and `error` null |
| 5 | Navigate to `{{base_url}}/#upcoming/99067d7f0917f050` and wait for "Acme renewal follow-up" | Tab "Prep" selected; section "In short" with the summary of step 2; "Expected from you" (Send the revised timeline · Nov 7, Send the revised timeline (2), Share the architecture diagram (2)); "Open with these people" (Question Which licences do we keep?); "Who's coming" Rita Gomez "2 calls"; the foot "Written <time of step 2> · <model>" and "Write it again" |

## Expected Final State
Acme renewal follow-up has a prep written on the local model, with its summary; nothing else changed in the data folder.

## Notes
- Verified 2026-11-07: 4 s on this Mac's Ollama (model loaded), about 1 s on the network test server; only `home/prep/99067d7f0917f050/prep.json` was written. The facts (expected, open, who's coming) come from the data, not the model.
- The automatic part of F-PREP-05 — the app writes preps by itself for today's meetings of open threads, at least 5 minutes before they start — is the app's watcher (`echoctl app`), not `window --serve`: not covered here.
