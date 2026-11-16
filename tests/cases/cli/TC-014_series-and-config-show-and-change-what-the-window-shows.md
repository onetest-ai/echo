---
id: TC-014
title: series list / set and config show and change what the window shows: a series' rule on its meeting's card, the settings in Settings
priority: medium
type: functional
requirements: [F-CLI-10]
module: cli
run: agent
platform: any
data: standard
tags: [cli, series, settings]
auto: [tests/auto/specs/S-28.spec.mjs]
---

# TC-014: series list / set and config show and change what the window shows: a series' rule on its meeting's card, the settings in Settings

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Reload the page fully (`{{base_url}}/`) after a CLI change before reading the window
- A meeting's block in the calendar's Day view is `[data-ev="<id>"]`; a click opens its card (`aside.detail`)

## Test Data

| Field | Before | Set to |
|---|---|---|
| Series `id:S-GLOBEX-WEEKLY` (Globex weekly, `ev-globex-weekly`) | record `ask`, no language | record `always`, languages `["uk"]` |
| `scheduled` (Calendar meetings) | `ask` | `manual` |
| `keep_audio` (Keep audio files) | `false` | `true` |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl series list` | JSON keyed by series: `id:S-ACME-WEEKLY` (`record` "always", `languages` ["en"]), `id:S-GLOBEX-WEEKLY` (`record` "ask", no `languages`), `id:S-INITECH-ALLHANDS` (`record` "never"), `subject:umbrella pricing call` (`record` "ask", `languages` ["uk"]) |
| 2 | Navigate to `{{base_url}}/#calendar/day`, click `[data-ev="ev-globex-weekly"]` | Under "Every meeting of this series" (drawn in capitals: match it ignoring case): "Ask" `aria-pressed="true"`, "Record" and "Don't record" `false`; the language chip "Default language ▾" |
| 3 | Run `target/debug/echoctl series set id:S-GLOBEX-WEEKLY '{"record":"always","languages":["uk"]}'` | `{"rule":{"languages":["uk"],"record":"always","subject":"Globex weekly","updated_at":…}}` |
| 4 | Run `target/debug/echoctl series list --thread globex-pilot` | `series` has one entry: `key` "id:S-GLOBEX-WEEKLY", `subject` "Globex weekly", `rule.record` "always", `rule.languages` ["uk"] |
| 5 | Reload, open `#calendar/day`, click `[data-ev="ev-globex-weekly"]` | "Record" pressed, "Don't record" not; the chip reads "Ukrainian ▾" |
| 6 | Run `target/debug/echoctl config` | JSON of the settings with `scheduled` "ask", `keep_audio` false, `me` `{"name":"Sam Rivera","email":"sam@initech.example"}`, `languages` ["en","uk","ru"] |
| 7 | Run `target/debug/echoctl config --scheduled manual --keep-audio true` | The printed settings have `scheduled` "manual" and `keep_audio` true |
| 8 | Reload, navigate to `{{base_url}}/#settings/recording` | In the "Calendar meetings" row "Manual" is pressed and "Ask" is not |
| 9 | Navigate to `{{base_url}}/#settings/transcription` | The switch "Keep audio files" is on (`aria-checked="true"`); read the switch, not its row's hint, which always describes Off ("Off: the recordings are deleted…") |
| 10 | Run `target/debug/echoctl series set id:S-GLOBEX-WEEKLY '{"record":"ask","languages":null}'` and `target/debug/echoctl config --scheduled ask --keep-audio false` | The rule is `ask` without languages again; `scheduled` "ask", `keep_audio` false |

## Expected Final State
Both the series rule and the settings are as they were; the window showed each change after a reload.

## Teardown
- Step 10 restores the data

## Notes
- A series rule with two languages (`["en","uk"]`) shows only the first on the card's chip ("English ▾"): the chip is a single choice. Not asserted here.
- `series set` with an unknown key, and the card's own buttons, are in `calendar/TC-018`, `calendar/TC-019`.
