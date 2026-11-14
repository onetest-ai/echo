---
id: TC-040
title: A plugin's work rows sit on My work among Echo's tasks (late or due this week, late first, its undated ones last, its icon, no Close or Claude) without a reload, in their thread's Work tab once each in every layout, and in ⌘K; their links open on the first click; one that fails keeps its rows marked "not updated" with a quiet line in Settings → Plugins; removed, they go
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
- The test plugin `tests/fixtures/plugins` (marketplace "test-plugins", plugin "work-demo"): for the user, "Review: fix login" (review · acme-test/api, due two days ago, of the thread `acme-rollout`), "Flaky login test <b>" (assigned · acme-test/web, due today) and "Tidy the labels" (assigned, no date); for the thread `acme-rollout`, "Bump dependencies" (PR · acme-test/api) and the same review; a "Snooze" action on its own rows; commands `fail` and `ok`

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin), `<data>/.echo/plugins/work-demo/`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install work-demo --yes` | The sheet lists "Adds its items to Today → My work and to threads' Work"; "Installed Work demo 0.1.0" |
| 2 | `echoctl work show`; `echoctl work show --thread acme-rollout` | The first: `rows` "Review: fix login" (`late: true`, `source: "work-demo"`, `icon: "plugin/work-demo/icon.svg"`), "Flaky login test <b>", then "Tidy the labels" (no date, last), `errors` `{}`; the second: the rows `pr-7` and `pr-8`, each once |
| 3 | Navigate to `{{base_url}}/#today`; wait up to 5 s, no reload (Today reads itself again while the plugin's first answer is on its way) | My work · this week lists "Review: fix login" with the late icon among the late tasks, "Flaky login test <b>" (the `<b>` as text) by its date, and "Tidy the labels" (no date) last, after every dated row — a plugin's undated item is on My work, an undated Echo task still isn't; each with the plugin's icon (hint "From Work demo"), "review · acme-test/api" / "assigned · acme-test/web", a "Snooze" button, and no Close or Claude icon; the row's pointer is the default arrow, its title a link button |
| 4 | Hover the title "Review: fix login"; click it once | Its tooltip "Open it in the browser (git.acme.example)"; the click opens it at once (the app's `open_url`), nothing asked (no "in the browser?" line under the row): with the test data's `ECHOCTL_TEST_OPEN_LOG_ONLY` the URL `https://git.acme.example/pr/7` is the last line of `<dir>/open.log` and no browser opens |
| 5 | Open "Actions ⌘K", type `fix login` | An entry "Open: Review: fix login" with the plugin's icon (hint "Work demo · git.acme.example"); Enter closes ⌘K and opens it at once: `https://git.acme.example/pr/7` appended to `<dir>/open.log` again, nothing asked |
| 6 | Navigate to `{{base_url}}/#project/acme-rollout/work`; wait up to 10 s | Under the bar: "Bump dependencies" and "Review: fix login", once each, with the plugin's icon; the board's columns don't hold them. Switch the layout to Tree: the two rows are still there; back to Board |
| 7 | Navigate to `{{base_url}}/#project/globex-pilot/work` | No row of the plugin |
| 8 | In the page's console: `fetch('api/plugin/work-demo/call/fail?args=%7B%7D')`; navigate to `{{base_url}}/#today`; wait up to 5 s | Today draws with the plugin's three rows still there, each marked "not updated" (hover: "Not updated: work-demo is failing on purpose") |
| 9 | Navigate to `{{base_url}}/#settings/plugins`, open "Work demo" | A quiet line "Its items in My work weren't updated the last time: work-demo is failing on purpose" |
| 10 | `fetch('api/plugin/work-demo/call/ok?args=%7B%7D')`; Today again; wait up to 5 s (a failed plugin's rows are kept a day after its last good answer, then dropped: not run here) | Its three rows, no "not updated" mark |
| 11 | Click "Snooze" on "Flaky login test <b>" | Within a few seconds, no reload: the row is gone from My work, with the notice "Snoozed Flaky login test <b>" |
| 12 | `echoctl plugin remove work-demo --delete-data`; reload Today and Acme rollout's Work | No row of the plugin anywhere; `{{base_url}}/plugin/work-demo/icon.svg` answers 404 |

## Expected Final State
Work demo is removed; My work and the threads' Work tabs hold only Echo's own tasks; no ledger changed.

## Teardown
- `echoctl plugin marketplace remove test-plugins`
- `tests/fixtures/serve.sh down <dir>`

## Notes
- A row's title opens its https page in the browser (the app's `open_url`) on its first click, as a plugin widget's link (plugin-authoring H17). With the test data's `ECHOCTL_TEST_OPEN_LOG_ONLY` set (`<dir>/env`) the app never opens it, on any OS: the URL is appended to `<dir>/open.log` (`ECHOCTL_TEST_OPEN_LOG`) — the case checks that line. Never run step 4 or 5 without the variable set.
- Today, open, reads itself again every minute; a thread's Work tab every 10 s.
