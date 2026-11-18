---
id: TC-029
title: Recall is in the activity bar only while a plugin adds a Knowledge tab and shows only those tabs; #brain, #brain/sources and #brain/people open Calls without one and the plugin's tab with one
priority: medium
type: functional
requirements: [F-KNOW-01, F-KNOW-06]
module: window
run: agent
platform: any
data: standard
tags: [window, recall, knowledge, plugins, routes]
---

# TC-029: Recall shows only a plugin's Knowledge tabs, and its old addresses land safely

**Module:** Window | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window
  served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps run in Bash after `. <dir>/env`, with
  `ECHOCTL_CONFIG_DIR` pointing into `<dir>`; the browser 1201 × 868
- A made-up local marketplace `acme-know` with one plugin, **Acme shelf**, that has only a window half adding a
  Knowledge tab. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market-know"; mkdir -p "$M/plugins/acme-shelf"
echo '{"name":"acme-know","plugins":[{"id":"acme-shelf","source":"./plugins/acme-shelf"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-shelf","name":"Acme shelf","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js"}' > "$M/plugins/acme-shelf/echo-plugin.json"
cat > "$M/plugins/acme-shelf/client.js" <<'EOF'
export function apply(ctx) {
  ctx.slots.add('knowledge.tab', { id: 'notes', label: 'Acme shelf', render: () => '<p>Acme shelf: the vault is empty.</p>' })
}
EOF
target/debug/echoctl plugin marketplace add "$M" --name acme-know
```

**Changes:** `<config dir>/brain.json` (`enabled`), `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl brain set --enabled true`; reload `{{base_url}}/`, navigate to `{{base_url}}/#today`, wait for "Your day" | The navigation "Activities" has no button "Recall": Intake on alone doesn't bring it |
| 2 | Navigate to `{{base_url}}/#brain`, then `{{base_url}}/#brain/sources`, then `{{base_url}}/#brain/people` (each a full load, each waited for a moment) | Each opens Calls (a call or "Pick a call"); no heading "Recall", no "Sources" or "People" tab, no error |
| 3 | `target/debug/echoctl plugin install acme-shelf --yes`; reload `{{base_url}}/`, wait for "Your day" | The navigation "Activities" has a button "Recall" between Tasks and Settings |
| 4 | Click "Recall" | The URL is `{{base_url}}/#brain`; the heading "Recall" with one tab "Acme shelf" (selected) and the text "Acme shelf: the vault is empty."; no tab "Sources" or "People", no scope picker, no "Settings" button |
| 5 | Navigate to `{{base_url}}/#brain/sources` (a full load), then `{{base_url}}/#brain/people` | Each shows the tab "Acme shelf" selected with its text: an address of a tab Echo no longer has opens the plugin's tab |
| 6 | With the page still on Recall: `target/debug/echoctl plugin remove acme-shelf --delete-data`; wait 4 s; then reload `{{base_url}}/#brain` | Within 4 s the open page leaves Recall for Calls and the button "Recall" is gone; after the reload Calls opens again |

## Expected Final State
Acme shelf is removed; Intake is on (the teardown turns it off).

## Teardown
- `target/debug/echoctl brain set --enabled false`; `target/debug/echoctl plugin marketplace remove acme-know`
- `tests/fixtures/serve.sh down <dir>`; close the test tab

## Notes
- The Recall activity has no content of its own: the browse screens (Sources, People) are gone, people and relations
  are the Knowledge plugin's. The dictionary is Settings → Vocabulary's (`vocabulary/TC-010`).
