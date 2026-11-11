---
id: TC-032
title: A widget's settings — Threads at a glance and Done this week count this week or this month; a plugin's widget declares its own (here a branch) and gets them; set in edit mode (⚙) or with echoctl today set, refused when not one it takes, kept on Today
priority: medium
type: functional
requirements: [F-TODAY-23]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, settings]
auto: [tests/auto/specs/S-24.spec.mjs]
---

# TC-032: A widget's settings

**Module:** Today | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 900
- The test plugin installed as in today/TC-029 step 1 (`widget-demo` 0.4.0)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl today add core:tasks.done --size S --above core:calendar.agenda`, `today add plugin:widget-demo:builds --size M --after core:tasks.done`; open `{{base_url}}/#today` (reload once if Last builds hasn't answered) | "Done this week", "N tasks since Monday"; Last builds' first line "Branch main" (its default) |
| 2 | `target/debug/echoctl today set core:tasks.done period year` | Refused: "core:tasks.done's period doesn't take \"year\" (week, month)", exit 1 |
| 3 | Edit Today; on Done this week click ⚙ ("Done this week: settings") | Under its header "Period" with week selected |
| 4 | Choose month | Done this week becomes "Done this month", "N tasks this month" (N at least the week's) |
| 5 | On Last builds click ⚙, set Branch to release; Done | Its first line "Branch release" (the plugin was asked again with it); `target/debug/echoctl today` shows `params` {"period": "month"} on Done and {"branch": "release"} on Last builds |
| 6 | `target/debug/echoctl today set core:tasks.done period week`; reload | Back to "Done this week" |
| 7 | `target/debug/echoctl today reset`; `target/debug/echoctl plugin remove widget-demo` | Today as it comes |

## Notes
- Checks and defaults are the Rust tests `today_layout::a_widgets_settings_are_kept_on_today_checked_and_filled_from_its_defaults` and `jshost` (a plugin's settings given to its provider; manifest checks).
