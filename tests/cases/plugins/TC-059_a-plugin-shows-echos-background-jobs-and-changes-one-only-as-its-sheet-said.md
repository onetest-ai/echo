---
id: TC-059
title: A plugin that asked for Echo's jobs shows them and changes one on the user's click; one that didn't, or is off, gets nothing
priority: high
type: functional
requirements: [F-PLUG-42]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, jobs, settings]
---

# TC-059: A plugin shows Echo's background jobs and changes one only as its sheet said

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, served: `tests/fixtures/serve.sh up <dir> <port> --set standard`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`) after `. <dir>/env`
- The test marketplace: `$E plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** the installed plugins, `<config dir>/jobs.json`

## Test Data

| Plugin | Jobs demo (`jobs-demo`) 0.1.0: `uses.echo.jobs: true`, a client half only (a Settings section "Jobs demo") |
|---|---|

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install jobs-demo --yes`; `$E plugin install signin-demo --yes` | Jobs demo's sheet has the line `• Shows Echo's background jobs and changes how often each runs, when it pauses at night and which model it uses — only when you change them in its screens (new)`; both installed |
| 2 | Navigate to `{{base_url}}/#settings/jobs-demo:main`; wait 2 s | "7 jobs: Needs you now, Thread sync, Threads' mail and meeting chats, Minutes, Meeting prep, Recall, Live notes"; "Needs you now every 5 min"; buttons "Every 10 min" and "Every 0 min" |
| 3 | Click "Every 10 min"; wait 2 s | A notice "Needs you now: every 10 minutes"; the row "Needs you now every 10 min"; `$E jobs`: `needs` every 10, `set` true |
| 4 | Click "Every 0 min"; wait 2 s. Then, in the page: `(await fetch('api/plugin/jobs-demo/jobs/set?job=needs')).status` | A notice "needs every: 0 is outside 1–240"; the row still "every 10 min"; then `404`: a change never goes by a GET (only the page's IPC) |
| 5 | In the page: `await (await fetch('api/plugin/signin-demo/jobs')).json()` | `{"error": "signin-demo didn't ask for “echo” when it was installed"}` |
| 6 | `$E plugin disable jobs-demo`; in the page: `await (await fetch('api/plugin/jobs-demo/jobs')).json()` | `{"error": "jobs-demo isn't on"}`; `$E jobs`: `needs` still every 10 (the plugin off changes no job) |

## Expected Final State
Needs you now every 10 minutes; Jobs demo installed and off.

## Teardown
- `$E jobs set needs every=`; `$E plugin remove jobs-demo`; `$E plugin remove signin-demo`; `tests/fixtures/serve.sh down <dir>`

## Notes
- A change without the user's click or key is refused in the page before anything is sent (`ctx.echo.jobs.set` asks `copyAllowed` first: `node crates/echoctl/ui/copy-guard.test.mjs`). Not checked here in the window: a browser driven by Playwright gives every page script the user's activation.
- What this checks is what Echo can check: the plugin a request names is on and asked for `uses.echo.jobs`, and a change comes only through the page's IPC on a click. Every plugin's client half shares Echo's page, trusted with the user's rights (its sheet says "Runs its own code in the Echo window, with your rights"), so one could name another plugin or send the IPC itself; that is the client halves' trust, not a check here.
