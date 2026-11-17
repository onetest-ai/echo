---
id: TC-143
title: "Each plugin's Claude session has its own tab: another plugin's session opening never takes the first one's away, and both are found again after the window reopens"
priority: medium
type: functional
requirements: [F-PLUG-94]
module: plugins
run: agent
platform: macos
data: small
ai: fake
tags: [plugins, claude-code, terminal]
---

# TC-143: Each plugin's Claude session in its own tab

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Everything of `plugins/TC-140`'s preconditions (the served data, the Bash prelude with `D` and `E`, the profile `acme-work`, the fake `claude`, the folder `$D/vault`, the plugin Acme ask in `$M`, the IPC helpers), then its step 1 (Acme ask installed with `$D/vault` given to it, Ask each time); no session open (`close`)
- A second made-up plugin, **Acme ask B**: Acme ask's files under another id and name, its buttons and its `#acme-ask-b` its own (Acme ask hears clicks on the whole page, so the two must not share them), in its own local marketplace `acme-ask-b`, given the same folder:

```bash
M2="$D/market-ask-b"; mkdir -p "$M2/plugins"; cp -R "$M/plugins/acme-ask" "$M2/plugins/acme-ask-b"
sed -i '' 's/"id":"acme-ask"/"id":"acme-ask-b"/; s/"name":"Acme ask"/"name":"Acme ask B"/' "$M2/plugins/acme-ask-b/echo-plugin.json"
sed -i '' 's/acme-ask/acme-ask-b/g; s/acmeAsk/acmeAskB/g' "$M2/plugins/acme-ask-b/client.js"
echo '{"name":"acme-ask-b","plugins":[{"id":"acme-ask-b","source":"./plugins/acme-ask-b"}]}' > "$M2/echo-marketplace.json"
$E plugin marketplace add "$M2" --name acme-ask-b; $E plugin install acme-ask-b --yes
ECHOCTL_PICK_FOLDER="$D/vault" $E plugin call acme-ask-b add
```

**Changes:** `<config dir>/plugins/` (Acme ask B, its marketplace and grant), `<home>/.echo/sessions.json`, `$D/run-*`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#acme-ask:main`; click "Ask the vault"; read `#acme-ask` and the bottom panel's tabs | `opened`; the bottom panel has a tab for Acme ask's session (`[data-panel-show="claude-plugin:acme-ask"]`), none for Acme ask B's |
| 2 | Navigate to `{{base_url}}/#acme-ask-b:main`; click "Ask the vault"; read `#acme-ask-b` and the bottom panel's tabs; `curl -s "$B/api/ptys"` | `opened`; the bottom panel has both `[data-panel-show="claude-plugin:acme-ask"]` and `[data-panel-show="claude-plugin:acme-ask-b"]`; the list has `claude:plugin:acme-ask` and `claude:plugin:acme-ask-b`, each `"exited":false` |
| 3 | Reload `{{base_url}}/` (the window reopened: sessions found running); read the bottom panel's tabs (open it with the status bar's "Claude" if it is closed); click the tab "Claude · Acme ask", read the panel; click the tab "Claude · Acme ask B", read the panel | The tabs "Claude", "Claude · Acme ask" and "Claude · Acme ask B"; the first shows `Acme ask · Claude Code in <$D/vault>, as that folder's own project — it reads only; asks each time` above its terminal (`acme fake session in <$D/vault>`), the second `Acme ask B · Claude Code in <$D/vault>, …` above its own |

## Expected Final State
Two sessions open in `$D/vault`, Acme ask's and Acme ask B's, each in its tab.

## Teardown
- `close`; `$E plugin remove acme-ask-b --delete-data`; `$E plugin marketplace remove acme-ask-b`; `rm -r "$M2"`; then `plugins/TC-140`'s teardown

## Notes
- Besides `echoctl`, this case runs `mkdir`, `cp`, `sed`, `echo`, `rm`, `curl` and `python3` (the helpers).
- A served window doesn't run the app's IPC answers, so before the reload a tab is named by the plugin's id; the app's own window names it at once.
