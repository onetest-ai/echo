---
id: TC-014
title: An installed plugin is enabled and its window part loads — a button in a slot (Call actions), a panel tab, a Settings section, a ⌘K entry; switching it off takes all of it out, on brings it back
priority: high
type: functional
requirements: [F-PLUG-04]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, install, enable, client-half, slots, palette]
---

# TC-014: An installed plugin's window part loads, and the switch takes it out and brings it back

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); CLI steps run in Bash after `. <dir>/env`; the browser 1201 × 868
- A made-up local marketplace `acme-local` with one plugin, **Acme greeter**, that has only a window half (no host half). The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-greeter"
echo '{"name":"acme-local","plugins":[{"id":"acme-greeter","source":"./plugins/acme-greeter"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-greeter","name":"Acme greeter","version":"0.1.0","summary":"A made-up plugin for tests","client":"client.js"}' > "$M/plugins/acme-greeter/echo-plugin.json"
cat > "$M/plugins/acme-greeter/client.js" <<'EOF'
export function apply(ctx) {
  ctx.slots.add('call.menu', { id: 'greet', label: 'Greet the call', run: () => ctx.go('tasks') })
  ctx.panels.add({ id: 'log', label: 'Acme greeter', render: () => '<p>Greeter panel: nothing logged yet.</p>' })
  ctx.settings.section({ id: 'main', label: 'Acme greeter', render: () => '<h2>Acme greeter</h2><p>Greetings are on.</p>' })
  ctx.palette.add({ label: 'Say hello (Acme greeter)', run: () => ctx.go('settings/acme-greeter:main') })
}
EOF
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-greeter --yes`; then `echoctl plugin list` | The first prints the sheet line `• Runs its own code in the Echo window, with your rights` and `Installed Acme greeter 0.1.0`; in the list's `installed`, the entry `acme-greeter` has `"enabled": true` and `"client": "client.js"` |
| 2 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#settings/plugins` (tab "Installed") | The group "Acme greeter" is a row with the switch "Acme greeter on" on (`aria-checked="true"`); the row has no "Has a problem" mark; the detail shows no text "Its window part didn't load" |
| 3 | In Settings' left list find the heading "From plugins"; click the button "Acme greeter" under it | The URL is `{{base_url}}/#settings/acme-greeter:main`; the heading "Acme greeter" and the text "Greetings are on." are shown |
| 4 | Navigate to `{{base_url}}/#calls`, click the row `[data-call$="acme-weekly-sync"]`, open "Call actions" | The menu lists the item "Greet the call" after "Ask Claude about this call" and before "Delete…" |
| 5 | Click the item "Greet the call" | The URL is `{{base_url}}/#tasks` (what the button was told to do) |
| 6 | Click "Actions ⌘K" (`[data-pal-open]`), type `hello` in "Action" | An entry "Say hello (Acme greeter)" is listed, in the group "Plugins" |
| 7 | Click that entry | The palette closes; the URL is `{{base_url}}/#settings/acme-greeter:main` |
| 8 | Open the right panel with the Inbox button (`[data-panel-flip="needs"]`; if the panel is already open, leave it); click the tab "Acme greeter" (`[data-panel-show="acme-greeter:log"]`) | The panel body reads "Greeter panel: nothing logged yet." |
| 9 | Navigate to `{{base_url}}/#settings/plugins`, click the switch "Acme greeter on" in the detail | It is off (`aria-checked="false"`) and the row looks dimmed; `echoctl plugin list` has `"enabled": false` for `acme-greeter` |
| 10 | Look at Settings' left list, the right panel's tabs, then `{{base_url}}/#calls` → the call → "Call actions", then "Actions ⌘K" with `hello` typed | Settings has no heading "From plugins" and no button "Acme greeter"; the right panel has no tab "Acme greeter"; the Call actions menu has no "Greet the call"; the palette lists no "Say hello (Acme greeter)" |
| 11 | Navigate to `{{base_url}}/#settings/plugins`, click the switch "Acme greeter on" again | It is on; `echoctl plugin list` has `"enabled": true` |
| 12 | Repeat the looks of step 10 | Settings has "From plugins" with the button "Acme greeter"; the right panel has the tab "Acme greeter"; the Call actions menu has "Greet the call"; the palette lists "Say hello (Acme greeter)" |

## Expected Final State
Acme greeter is installed and on, and all four parts of its window half (Call actions item, panel tab, Settings section, ⌘K entry) are in the window again.

## Teardown
- Close the palette (Escape); `echoctl plugin remove acme-greeter --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; close the test tab

## Notes
- The window half is trusted code loaded into the page (`apply(ctx)` in `33-plugins-client.js`); what it adds is taken out in `unloadPlugin` when the plugin is switched off or removed. Step 10 is the check of that, part by part.
- The plugin has no host half, so its sheet is the one line about the window; it asks for nothing else of Echo.
- The panel tab's name and the palette entry's text are the plugin's own (`label`); the palette's group name for a plugin that gives none is "Plugins" (drawn in capitals by the style).
