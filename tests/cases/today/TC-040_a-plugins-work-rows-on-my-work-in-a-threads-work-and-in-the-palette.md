---
id: TC-040
title: A plugin's work rows sit on My work among Echo's tasks (late first, its icon, no Close or Claude), in their thread's Work tab once each, and in ⌘K; one that fails is left out with a quiet line in Settings → Plugins; removed, they go
priority: medium
type: functional
requirements: [F-PLUG-20]
module: today
run: agent
platform: any
data: standard
tags: [plugins, today, my-work, threads, work, palette]
---

# TC-040: A plugin's work rows on My work, in a thread's Work and in ⌘K

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`; the browser 1201 × 868
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "work-demo"): for the user, "Review: fix login" (review · acme-test/api, due two days ago, of the thread `acme-rollout`) and "Flaky login test <b>" (assigned · acme-test/web, due tomorrow); for the thread `acme-rollout`, "Bump dependencies" (PR · acme-test/api) and the same review; a "Snooze" action on its own rows; commands `fail` and `ok`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<data>/.echo/plugins/work-demo/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install work-demo --yes` | The sheet lists "Adds its items to Today → My work and to threads' Work"; "Installed Work demo 0.1.0" |
| 2 | `echoctl work show`; `echoctl work show --thread acme-rollout` | The first: `rows` "Review: fix login" (`late: true`, `source: "work-demo"`, `icon: "plugin/work-demo/icon.svg"`) then "Flaky login test <b>", `errors` `{}`; the second: the rows `pr-7` and `pr-8`, each once |
| 3 | Navigate to `{{base_url}}/#today`; wait 3 s and reload (the first look asks the plugin in the background) | My work · this week lists both rows among Echo's tasks: "Review: fix login" with the late icon among the late tasks, "Flaky login test <b>" (the `<b>` as text) by its date; each with the plugin's icon (hint "From Work demo"), "review · acme-test/api" / "assigned · acme-test/web", a "Snooze" button, and no Close or Claude icon |
| 4 | Open "Actions ⌘K", type `fix login` | An entry "Open: Review: fix login" (hint "Work demo") |
| 5 | Escape; navigate to `{{base_url}}/#project/acme-rollout/work`; wait 3 s and reload | Above the board: "Bump dependencies" and "Review: fix login", once each, with the plugin's icon; the board's columns don't hold them |
| 6 | Navigate to `{{base_url}}/#project/globex-pilot/work` | No row of the plugin |
| 7 | In the page's console: `fetch('api/plugin/work-demo/call/fail?args=%7B%7D')`; wait 2 s; navigate to `{{base_url}}/#today` | Today draws, without the plugin's rows |
| 8 | Navigate to `{{base_url}}/#settings/plugins`, open "Work demo" | A quiet line "Its items for My work were left out the last time: work-demo is failing on purpose" |
| 9 | `fetch('api/plugin/work-demo/call/ok?args=%7B%7D')`; wait 2 s; Today again | Both rows are back |
| 10 | Click "Snooze" on "Flaky login test <b>"; wait for the next refresh (or reload) | That row is gone from My work |
| 11 | `echoctl plugin remove work-demo --delete-data`; reload Today and Acme rollout's Work | No row of the plugin anywhere; `{{base_url}}/plugin/work-demo/icon.svg` answers 404 |

## Expected Final State
Work demo is removed; My work and the threads' Work tabs hold only Echo's own tasks; no ledger changed.

## Teardown
- `echoctl plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Clicking a row's title opens its https page in the browser (the app's `open_url`); a headless run checks the button's `data-open-url` instead.
