---
id: S-30
title: "Minutes and the work on a local model: no Claude Code, a template by title, write again with another, a memo, a thread's sync, Yolo, a prep, and a call's profile"
run: agent
data: standard
ai: local
cases:
  - tests/cases/calls/TC-037_without-claude-code-or-its-plugin-the-minutes-tab-says-what-to-do.md
  - tests/cases/templates/TC-018_minutes-follow-the-template-whose-words-are-in-the-title.md
  - tests/cases/claude/TC-016_write-again-with-another-template-rewrites-the-minutes-and-keeps-it.md
  - tests/cases/claude/TC-017_a-memo-gets-minutes-in-the-memo-template-by-thread.md
  - tests/cases/prep/TC-015_prep-write-on-the-local-model-writes-a-summary.md
  - tests/cases/tasks/TC-026_yolo-on-the-local-model-does-the-task-and-says-so-above-it.md
  - tests/cases/work/TC-028_sync-on-the-local-model-reads-a-call-and-keeps-what-you-set.md
  - tests/cases/calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md
---

# S-30: Minutes and the work on a local model

**Story:** Sam has no Claude account on this computer: his AI profile is a model server of his own (Ollama, on this Mac or one on the network). First he sees what Echo says on a computer without Claude Code or without Echo's plugin. Then Claude Code writes real minutes on the local model: a standup's follow the Status sync template because of its title, a negotiation's are written again with Discussion and keep it, a memo's come in the memo template, one part per thread. He has a meeting's prep written, gives a task to Claude with Yolo, syncs Acme rollout's work after editing two of its items himself, and last checks that a call keeps the profile it was given when he changes the default.

**Data:** `standard` with the local model (`tests/fixtures/serve.sh up <dir> <port> --set standard --ai local`; `tests/fixtures/README.md`, "Real Claude Code runs on a local model"). Fresh data per run, built right before it, between 06:00 and 20:00 local (the prep stage needs a meeting still to come). Needs Claude Code on `PATH` and the data's model server answering. Serve it with `serve.sh` only, never the Echo app. Ports: the scenario's, and its port + 1 and + 2 for stage 1.

**Length:** about 20 minutes of model time on the network test server (stage 2 ≈ 2 min, 3 ≈ 1 min, 4 ≈ 1 min, 5 seconds, 6 ≈ 4–7 min, 7 ≈ 5 min); about 1 h 45 min on a Mac running the model itself (7 ≈ 45 min, 6 may hit Yolo's 20-minute limit). One model job per stage; a stage whose run fails for an infrastructure reason (Ollama down, model missing) is BLOCKED, not retried.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `calls/TC-037_without-claude-code-or-its-plugin-the-minutes-tab-says-what-to-do.md` | — | Load `{{base_url}}/` (the cookie) |
| 2 | `templates/TC-018_minutes-follow-the-template-whose-words-are-in-the-title.md` | — | — |
| 3 | `claude/TC-016_write-again-with-another-template-rewrites-the-minutes-and-keeps-it.md` | — | — |
| 4 | `claude/TC-017_a-memo-gets-minutes-in-the-memo-template-by-thread.md` | — | — |
| 5 | `prep/TC-015_prep-write-on-the-local-model-writes-a-summary.md` | — | — |
| 6 | `tasks/TC-026_yolo-on-the-local-model-does-the-task-and-says-so-above-it.md` | — | — |
| 7 | `work/TC-028_sync-on-the-local-model-reads-a-call-and-keeps-what-you-set.md` | — | — |
| 8 | `calls/TC-038_a-call-keeps-its-profile-when-the-default-changes.md` | — | Reload `{{base_url}}/` |

## Notes
- Model output differs from run to run: the stages assert files, states, template names, headings and the window around the output, never its wording.
- Stage 4 renames the memo's folder (the run titles it); stage 7 changes Acme rollout's work; stage 8 changes the default AI profile and puts it back. Stage 2 must run before stage 3 reads the "Write again with" menus.
- Nothing here signs in to anything: the fake `claude` refuses every run not routed to the local model.
