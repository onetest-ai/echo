---
id: TC-149
title: A plugin switched off while its page is shown hands the window over to Calls, drawn once, with no error
priority: medium
type: functional
requirements: [F-PLUG-28]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, window, client-half]
---

# TC-149: A plugin switched off while its page is shown

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data served: `tests/fixtures/serve.sh up <dir> <port>`; CLI steps in Bash, each starting with `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; E=target/debug/echoctl` (inside the repository, built right before the run); abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- The made-up plugin **Acme page** (a window half with one page) in a local marketplace `acme-page`:

```bash
M="$D/market-page"; mkdir -p "$M/plugins/acme-page"
echo '{"name":"acme-page","plugins":[{"id":"acme-page","source":"./plugins/acme-page"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-page","name":"Acme page","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js"}' > "$M/plugins/acme-page/echo-plugin.json"
echo "export function apply(ctx) { ctx.activities.add({ id: 'main', label: 'Acme page', icon: '', render: () => '<h2>Acme page</h2>' }) }" > "$M/plugins/acme-page/client.js"
$E plugin marketplace add "$M" --name acme-page; $E plugin install acme-page --yes
```

**Changes:** `<config dir>/plugins/` (the marketplace and the plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#acme-page:main` | The heading "Acme page" |
| 2 | Leave the window on it; `$E plugin disable acme-page`; wait 5 s; read the URL, the page and the console's errors | The URL is `{{base_url}}/#calls`; the calls list ("Search calls") beside the call's pane; no heading "Acme page"; the console has no `TypeError` (only the page load's `favicon.ico` 404) |

## Expected Final State
Acme page installed and off; the window on Calls.

## Teardown
- `$E plugin remove acme-page`; `$E plugin marketplace remove acme-page`; `rm -r "$M"`

## Notes
- Besides `echoctl`, this case runs `mkdir`, `echo` and `rm`.
- The same hand-over follows a switch-off in Settings → Plugins or a removal; the rule (a view no activity has is drawn in Calls' layout) is `workbench.test.mjs`.
