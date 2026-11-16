---
id: TC-012
title: Each row's icon names the call's state: minutes, transcript, transcribing, failed, audio, memo
priority: medium
type: functional
requirements: [F-CALLS-02]
module: calls
run: agent
platform: any
data: standard
tags: [calls, list, status]
auto: [tests/auto/specs/S-23.spec.mjs]
---

# TC-012: Each row's icon names the call's state: minutes, transcript, transcribing, failed, audio, memo

**Module:** Calls | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- A call's row in the Calls list is `[role=option][data-call$="-<slug>"]` (the folder name ends in the slug; for Acme weekly sync the first match is today's call)

## Test Data

| Row (slug) | Icon `.si` aria-label | Other `.m` spans |
|---|---|---|
| Acme weekly sync (`acme-weekly-sync`, Today) | Minutes | 30 min |
| Initech planning (`initech-planning`) | Transcript | 25 min |
| Quick check-in (`quick-check-in`) | Transcribing | 15 min |
| Umbrella intro call (`umbrella-intro-call`) | Failed | 20 min |
| Recording (`recording`) | Audio | 12 min |
| Memo (`memo`) | Memo (class `si memo`, a microphone) | Memo, 4 min |
| Globex steering (`globex-steering`) | Minutes | Teams transcript, 45 min |
| Globex weekly (`globex-weekly`) | Transcript | Teams transcript, 10 min |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls` and wait for "Acme weekly sync" | The list shows the groups Today and Yesterday |
| 2 | Read the icon of the row Acme weekly sync | Its aria-label is "Minutes"; its `.m` holds "30 min" |
| 3 | Read the icon of the row Initech planning | "Transcript" |
| 4 | Read the icon of the row Quick check-in | "Transcribing" |
| 5 | Read the icon of the row Umbrella intro call | "Failed" |
| 6 | Read the icon of the row Recording | "Audio" |
| 7 | Read the icon and the `.m` spans of the row Memo | The icon's label is "Memo" and its class holds `memo`; the spans include "Memo" and "4 min" |
| 8 | Read the icon and the `.m` spans of the row Globex steering | The icon is "Minutes"; the spans include "Teams transcript" and "45 min" |
| 9 | Read the icon and the `.m` spans of the row Globex weekly | The icon is "Transcript" (an imported call has no minutes yet); the spans include "Teams transcript" |

## Expected Final State
Every row carries the icon of its state; memos show a microphone and "Memo", imported calls "Teams transcript".

## Notes
- Creates or changes nothing.
- The icon `Recording` (a red dot) needs a call being recorded by the app: not in the data set (`call.status=recording` is skipped).
