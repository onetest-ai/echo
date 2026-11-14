---
id: TC-042
title: "A plugin says its widget changed (ctx.widgets.changed) and Echo asks it again at once — from a terminal's plugin call too — while a widget with an hour-long refresh stays as it was until then"
priority: high
type: functional
requirements: [F-TODAY-19]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, widgets, events, host-half]
---

# TC-042: ctx.widgets.changed asks a widget again

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (the served window stands in for the app: it runs the plugins' host and takes what other processes queue for it); the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env` (`E="$PWD/target/debug/echoctl"`); the browser 1440 × 900
- The test plugin `pulse-demo` from `tests/fixtures/plugins` (marketplace "test-plugins"): a widget "Bumps" (`refresh` 3600 s) showing how many times it was bumped; commands `bump` (one more, then `ctx.widgets.changed('count')`) and `bumpQuiet` (one more, nothing said)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `$E plugin install pulse-demo --yes`; `$E today add plugin:pulse-demo:count --size S`; reload `{{base_url}}/#today` (once more if it hasn't answered) | "BUMPS" with 0, "bumps" |
| 2 | `$E plugin call pulse-demo bumpQuiet`; wait 3 s; reload | Still 0: nothing said it changed, and its refresh is an hour away (`$E plugin call pulse-demo bumpQuiet` answered 1) |
| 3 | `$E plugin call pulse-demo bump`; wait 3 s; reload | 2: the terminal's `bump` said it changed, the app took that within a second and asked the widget again |
| 4 | `$E plugin call pulse-demo bump`; at once `$E plugin call pulse-demo bump`; wait 15 s; reload | 4 (asked again at most once in 10 s, the last change within that time asked at its end — never left at 3) |
| 5 | `$E today reset`; `$E plugin remove pulse-demo --delete-data`; `$E plugin marketplace remove test-plugins` | Today as it comes |

## Teardown
- `tests/fixtures/serve.sh down <dir>`; close the browser tab

## Notes
- Each copy (Today's and each dashboard's) is asked with its own settings, changes within 10 s are asked once at the end, a `changed()` from inside the provider isn't heard, and another process's is marked due and queued for the app: the Rust test `jshost::tests::widgets_changed_asks_each_copy_again_with_its_own_settings_at_most_once_a_gap`.
- Today itself reads its widgets' data about once a minute; a reload shows the new answer at once.
