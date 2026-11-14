---
id: TC-044
title: A thread's and a client's dashboard — a band of widgets over the Brief and the Overview, offering only the widgets that show one thread (one client), the page setting which; edited as Today is; each page its own layout; Today untouched
priority: high
type: functional
requirements: [F-TODAY-28]
module: today
run: agent
platform: any
data: standard
tags: [today, widgets, plugins, dashboards, threads, clients]
---

# TC-044: A thread's and a client's dashboard

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (its `env` sets `ECHOCTL_TEST_OPEN_LOG_ONLY`); the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 1100
- The test plugins `table-demo` and `widget-demo` from `tests/fixtures/plugins` (marketplace "test-plugins"): `target/debug/echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `target/debug/echoctl plugin install table-demo --yes`; `target/debug/echoctl plugin install widget-demo --yes`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open `{{base_url}}/#project/acme-rollout/brief` | Above "What it is": only a ✎ button (its tooltip "Add widgets that show this thread"), no widgets |
| 2 | `target/debug/echoctl today --thread acme-rollout add plugin:table-demo:staffing` | Refused: "plugin:table-demo:staffing can't go on a thread's dashboard: it has no thread setting" |
| 3 | `target/debug/echoctl today --thread acme-rollout add plugin:table-demo:positions --size S`; then `today --thread acme-rollout set plugin:table-demo:positions thread globex-pilot` | The add prints the layout with `plugin:table-demo:positions` at S; the set is refused: "plugin:table-demo:positions's thread is this page's: it isn't set here" |
| 4 | Reload the thread's Brief (once more if the widget hasn't answered) | Over "What it is": "Open positions" with 3; `curl -s '{{base_url}}/api/dashboard?page=thread:acme-rollout'` has its data's note "thread acme-rollout · client acme" (the thread, and the thread's client) |
| 5 | Click ✎ | The band framed for editing, "Add a widget" and "Done" over it; the right panel "Add a widget" lists only Echo's own that show one thread — "Your day · agenda", "Your day · calendar", "Your day · strip", "Next meeting", "Done this week", "Recorded this week", "My work · this week" (TC-045, TC-046) —, "Open positions" ("here"), "Roles to fill" and "Last builds" (Add) — no "Threads at a glance", no "Needs attention"; Open positions has no ⚙ |
| 6 | In the panel click Add on "Last builds"; click its ⚙ | "Last builds" placed after Open positions; its settings show only "Branch" (no "Thread") |
| 7 | Choose Branch "release"; click ↑ on Last builds' frame; switch to the Work tab and back; click Done | After ↑ Last builds is first and the keyboard focus stays on its frame; on Work no dashboard and no Add a widget panel; after Done no frames, "Last builds" shows "Branch release" as its first line |
| 7a | `target/debug/echoctl today add plugin:table-demo:positions --size S`; open `{{base_url}}/#today`; then open `{{base_url}}/#project/globex-pilot/brief`, click ✎, hover "Open positions" in the panel, click its Add, wait a moment | Today: "Open positions" 12 (every thread). Globex pilot's dashboard: "Open positions" with 3 (its own answer, never Today's 12); click Done; `target/debug/echoctl today --thread globex-pilot remove plugin:table-demo:positions` |
| 8 | Open `{{base_url}}/#client/acme/overview` | Above "About Acme": only ✎ (nothing on this page yet) |
| 9 | `target/debug/echoctl today --client acme add plugin:table-demo:positions --size S`; reload (once more if it hasn't answered) | "Open positions" with 7; its note in `api/dashboard?page=client:acme` "every thread · client acme" |
| 9a | `target/debug/echoctl today --client acme add plugin:table-demo:roles --size M`; reload; on "Roles to fill" click the first row's ▸; click "Open" (its foot) once; click "Nudge" | The row's details show (Dana Example, Lee Sample, Ira Placeholder); Open opens it at once, nothing asked (in `<dir>/open.log`, nothing opened for real); after Nudge no error shows and, in the page, `await get('api/plugin/table-demo/call/nudged?args=%7B%7D')` gives `{value: 1}` (the action ran in the app; `echoctl plugin call` would run its own copy) |
| 9b | `target/debug/echoctl today --client acme remove plugin:table-demo:roles`; `today --thread acme-rollout add plugin:table-demo:roles --size M`; open `{{base_url}}/#project/acme-rollout/brief`; repeat 9a's clicks | The same on a thread's dashboard: details, Open straight to open.log, `nudged` 2 the same way; the row's ▸ opens here though it was open on the client's page (each page its own); then `today --thread acme-rollout remove plugin:table-demo:roles` |
| 10 | Open `{{base_url}}/#project/globex-pilot/brief`; click ✎ | Only ✎ before; editing: "Nothing here yet: Add a widget puts one that shows this thread."; the panel offers its three plugin widgets with Add (Open positions, Roles to fill, Last builds) (each page its own layout). Click Done |
| 11 | `target/debug/echoctl today remove plugin:table-demo:positions`; open `{{base_url}}/#today`; `target/debug/echoctl today` | Today as it comes: no "Open positions", no "Last builds"; `today` lists none of the plugins' widgets |
| 12 | `target/debug/echoctl today --thread acme-rollout reset`; `today --client acme reset`; reload the thread's Brief | `[]` each; only ✎ above "What it is" |

## Teardown
- `target/debug/echoctl plugin remove table-demo`; `target/debug/echoctl plugin remove widget-demo`; `target/debug/echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`; close the browser tab
