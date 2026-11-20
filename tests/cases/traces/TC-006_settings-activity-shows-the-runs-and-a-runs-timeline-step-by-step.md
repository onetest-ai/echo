---
id: TC-006
title: Settings → Activity shows what Echo ran — the facts, tokens a day, by job, the runs — and a run's timeline step by step with what went in and came out
priority: high
type: functional
requirements: [F-TRACE-07, F-TRACE-08]
module: traces
run: agent
platform: macos
data: standard
ai: virtual
tags: [traces, activity, settings, window, virtual-model]
---

# TC-006: Settings → Activity, and a run's timeline

**Module:** Traces | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set with the virtual model, served: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai virtual`; `{{base_url}}/` loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env` (abort if `ECHOCTL_HOME` isn't inside `<dir>`)
- On this data (S-33's stages before this one): Acme renewal follow-up's prep written (`prep/TC-015`), Acme rollout's sync run (`work/TC-028`), Yolo runs done, one of them failed (`tasks/TC-032`), and a failed command (`traces/TC-005`)
- The window at least 1280 × 800

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#settings/activity` | Settings with "Activity" chosen in its list (between Plugins and Layout); the heading "Activity", "What Echo ran, step by step, on this computer — each run's detail kept 7 days, its numbers 90"; "Today", "7 days" (pressed), "30 days", "90 days" |
| 1b | Click "90 days", then "7 days" | 90 days: the chart has 90 bars, dated on Mondays and "Today"; the same runs (all are from today); 7 days again |
| 2 | Read the facts and the chart | "N runs · M failed" (M at least 1: the failed Yolo run), tokens with "% from cache", "$… estimated, as Claude Code counts", requests to other services; "Tokens a day" with seven days ending "Today", today's bar in the colours of its legend (Thread sync, Minutes, Live notes, Other) |
| 3 | Read the table by job | Rows "Thread sync", "Yolo" (with a red mark: a failed run), "Model call · virtual", each with its runs, tokens, cost and model; most tokens first |
| 4 | Click "Failed" above the runs, then "All" | Failed: only the failed Yolo run and "echoctl traces failed …" (each with a red dot); All: every run again, newest first |
| 5 | Click the run "Thread sync · …" | It is marked; the right panel "Run": "Thread sync", its time and "3 Claude Code runs", "Open timeline" and "Copy id", Model "virtual", Tokens, Cost "… estimated", Steps: two "Claude Code · a thread's sync — extract" (the answer and its fix run) and one "— verify", with their tokens; the table and the runs now stack one above the other, nothing cut |
| 6 | Click "Open timeline" | "Activity › Thread sync · HH:MM"; the heading "Thread sync · 1 read" with "Today HH:MM:SS · … · 3 Claude Code runs · … tokens"; the timeline: "Thread sync", under it the three "Claude Code · a thread's sync — …" rows (extract, extract, verify), under each one "Request · virtual" row with tokens, no tool rows; an axis in seconds; the colours' line below |
| 7 | Click "Activity" in the path; click the Yolo run that did its task (`tasks/TC-026`), "Open timeline", then its first red "Bash · …" row | The timeline holds its tool rows, the refused ones in red; the right panel "Step": "Bash", "Took", "Tool Bash" with that command, "Refused" with the words the run was told; then back: "Activity", the Thread sync run, "Open timeline" |
| 8 | Click the extract's "Claude Code · a thread's sync — extract" row | The panel: Took, Tokens, Cost, Model "virtual", Profile "ollama", Turns, Telemetry "Claude Code reports to Echo"; "What went in and came out": Instructions (append-system-prompt-file) starting "# Echo: a thread's sync — extract", In (the thread's evidence), Claude's text (the answer, a JSON object), Result; no Tool entries |
| 9 | Click "Activity" in the path | Back to the list, the run still marked |

## Expected Final State
Nothing changes.

## Teardown
- None (the scenario's teardown stops the server)

## Notes
- The sums, filters, labels, timeline rows and a step's details: node test `crates/echoctl/ui/activity.test.mjs`.
