---
id: TC-022
title: A call made from a Teams transcript says so and shows its turns with the transcript's speaker names and "Me" for the user
priority: medium
type: functional
requirements: [F-TR-06]
module: window
run: agent
platform: any
data: standard
tags: [transcript, teams-import]
auto: [tests/auto/specs/S-29.spec.mjs]
---

# TC-022: A call made from a Teams transcript says so and shows its turns with the transcript's speaker names and "Me" for the user

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); file reads run after `. <dir>/env`
- Globex steering (today, about 6 h before the build) was made from its Teams transcript: `meta.json` `source` is the Teams transcript, no audio; its `segments.json` holds the turns as the import writes them (`speaker` "me" for the user — About me's name in the transcript — else "remote" with the transcript's `name`)

## Test Data

| Turn | segments.json | Shown |
|---|---|---|
| 0:05 | `speaker` "me" | "Me" + language chip "en" |
| 0:17 | `speaker` "remote", `name` "Speaker 1" | "Speaker 1" + "en" |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="-globex-steering"]` | The header reads "Globex steering", "45 min", the kind chip "Client ▾", "from Teams transcript" (hint: "Made from the meeting's Teams transcript: no audio, no live notes"), the people "Kim Park, Noor Aziz, Ben Okafor", the thread chip "Globex pilot" |
| 2 | Click the tab "Transcript" | Two turns: `.turn.me` at 0:05 with `.who` "Me" (chip "en") and "Welcome to the steering meeting."; `.turn.remote` at 0:17 with `.who` "Speaker 1" (chip "en") and "We approve the pilot scope." |
| 3 | Run `cat "$ECHOCTL_HOME/calls/$(ls "$ECHOCTL_HOME/calls" \| grep -- '-globex-steering$')/segments.json"` | The two turns of the Test Data; the window showed each one's name as stored |
| 4 | Read the tabs | "Minutes" and "Transcript" only: no Live tab for an import |

## Expected Final State
Nothing changed.

## Notes
- The import itself (Teams' VTT → turns, the user's name from About me → "Me", the others' names from the transcript) needs Microsoft 365 and a meeting Teams transcribed: person-run (`calendar/` Microsoft 365 cases); its parsing is covered by the Rust tests of `teams_transcript.rs`.
- The test data's import names its other speaker "Speaker 1", not a person's name; a set that holds a named speaker from a transcript (e.g. "Kim Park") would let this case check names proper.
