---
id: TC-027
title: The sync line says synced with calls to read and items to review, failed, or not synced yet
priority: medium
type: functional
requirements: [F-WORK-03, F-WORK-08]
module: work
run: agent
platform: any
tags: [work, sync, review]
---

# TC-027: The sync line says synced with calls to read and items to review, failed, or not synced yet

**Module:** Work | **Priority:** Medium | **Type:** Functional

## Preconditions
- `data: standard`: the standard data set, built and served by `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window is at `{{base_url}}` and `{{base_url}}/` is loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`. Times in the data are relative to the run's start, so build it right before the run
- No sync has been started

## Test Data

| Thread | Thread id | Sync state | Calls to read | To review | Line |
|---|---|---|---|---|---|
| Acme rollout | acme-rollout | done | 3 | 3 (T004 merge, T006 move, T007 weak "done") | Synced HH:MM · 3 calls to read · 3 to review |
| Globex pilot | globex-pilot | failed | 5 | 0 | Sync failed |
| Umbrella proposal | umbrella-proposal | never | 0 | 0 | Not synced yet |
| Acme renewal | acme-renewal | never | 0 | 0 | Not synced yet |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#project/acme-rollout/work` and read the text at the right of the bar (`.wk-sync`) | It reads "Synced " and a time (HH:MM), then "3 calls to read" and "3 to review" (the last in amber); the button "Sync now" is enabled (not pressed) |
| 2 | In Acme rollout read the list above the board ("To review") | Three lines: "Same work as … ?" for T004 with Yes and No, "Belongs under … ?" for T006 with Yes and No, "Is Answer the security questionnaire done?" with Done and Look; none is pressed |
| 3 | Navigate to `{{base_url}}/#project/acme-renewal/work` and read `.wk-sync` | It reads "Not synced yet" |
| 4 | Navigate to `{{base_url}}/#project/umbrella-proposal/work` and read `.wk-sync` | It reads "Not synced yet" |
| 5 | Navigate to `{{base_url}}/#project/globex-pilot/work` and read `.wk-sync` | It reads "Sync failed" in red, with no count of calls to read although 5 wait; hovering it gives "Claude Code exited with an error (exit 1)" |

## Expected Final State
The four threads show their lines; nothing was pressed.

## Teardown
- None: the case only reads

## Notes
- Never press "Sync now": it starts Claude Code. The data's fake `claude` would refuse it, but the case only reads.
- Globex pilot hides its 5 calls to read while the sync failed (the line shows only "Sync failed"). Verify at run time whether it should say both.
