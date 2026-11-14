---
name: build
description: "Use when the user wants a widget, chart, dashboard or plugin of their own in Echo — \"make me a widget that shows…\", \"put a chart of … on my Today\", \"a dashboard for the Acme thread\", \"build a plugin that…\", \"change my widget\" — even without saying Echo. Makes (or changes) the user's own Echo plugin in My plugins, their Echo data folder; checks its widgets' data; installs or updates it once the user agrees to what Echo's permission sheet says; places the widgets on Today or a thread's or client's dashboard. Installing others' plugins or setting one up: the manage skill."
argument-hint: "[what it should show, and where]"
---

# echo: build

The user's own plugins live in **My plugins**: a folder in their Echo data folder, one subfolder a plugin, its
folder name its id. Echo offers each there to install, and each change to its files as an update — always with the
permission sheet when it asks for something new. Echo installs a copy; editing the folder changes nothing until updated.

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`), with plain words — no pipes, redirects or `$`.
Inside a run Echo started (`ECHOCTL_HEADLESS` is set) do nothing: say this is done in the user's own session.

## 1. What, where, how big

Settle with the user, in a line each: what the widget shows (a number, a few numbers, a list, a table, a chart —
which type), where its data comes from (a web API, a file or folder on this computer, Echo itself), how often it
changes, and where it goes: Today, or one thread's or client's dashboard (a widget there needs a `thread` or
`client` setting — the page fills it in). Ask only what you can't infer.

## 2. The rules

`echoctl plugin guide` prints Echo's plugin authoring rules (the manifest, `uses` and the permission sheet,
the widget kinds and their data — `stat`, `stats`, `chart` of every type, `list`, `status`, `card`, `table` —
the host half's `ctx`, settings, secrets). Read the parts you need before writing; follow them exactly. Never
put a secret (token, password) in a file: declare a setting of type `secret` and let the user fill it in Settings.

## 3. The plugin

`echoctl plugin mine` gives the folder (`folder`) and what Echo reads there. Write only inside
`<folder>/<id>/`, `id` lower-case letters, digits and `-` (check `plugins` there that it isn't taken). The least that
works — `echo-plugin.json`:

```json
{"id": "team-pulse", "name": "Team pulse", "version": "0.1.0", "summary": "Open items by owner",
 "main": "main.js",
 "widgets": [{"id": "open", "label": "Open items", "about": "how many are open", "kind": "stat", "sizes": ["S", "M"]}]}
```

and `main.js` (the host half: one file, no imports):

```js
export function apply(ctx) {
  ctx.widgets.provide('open', async ({ size, params }) => {
    return { value: 12, label: 'open items' }
  })
}
```

A chart: `kind: "chart"` and data `{type, points, series?, headline?}` (the guide's R26). A widget for a thread's or
client's dashboard: `params: [{key: "thread", label: "Thread", type: "thread"}]` (or `client`); read it from `params`.
Ask for a `uses` line (network hosts, folders, Claude…) only when the widget needs it — each is a line the user agrees to.

## 4. Check, install

1. `echoctl plugin mine` — your plugin is listed with no `error` (else fix what it says). (No data folder chosen yet:
   it says so — the user picks one in Settings first.)
2. `echoctl plugin install <id> --from my-plugins` — without `--yes`: Echo prints what it asks (the sheet). Tell the user
   those lines in your own words and ask; only after their yes, run it again with `--yes`.
3. `echoctl widget show plugin:<id>:<widget>` — the data Echo gets from your provider; fix the plugin until it has no
   `error` and shows what the user wanted. (A change → `echoctl plugin update <id>`; when it asks for something new,
   it stops and prints it — `--yes` only after the user's yes.) If Echo wasn't running, it picks the plugin up when it starts.

## 5. Place it

- On Today: `echoctl today add plugin:<id>:<widget> --size M` (`--above`, `--below`, `--before`, `--after <widget>`
  to say where; `echoctl today` lists what is there).
- On a thread's or client's dashboard: the same with `--thread <thread id>` or `--client <client id>`
  (`echoctl project list`, `echoctl client list` give the ids).
- On a dashboard of its own beside every thread's Brief (every client's Overview), e.g. "a Staffing dashboard for my clients": `echoctl dashboard add --client Staffing` (it prints its id), then the same with `--client <a client id> --board <its id>` — its layout is every client's, each showing its own
- A setting: `echoctl today set plugin:<id>:<widget> <key> <value>` (with `--thread` / `--client` for a dashboard).

Say where it is and how to change it: Edit on Today or ✎ on the dashboard; or ask you again.

## Changing it later

Edit the files in `<folder>/<id>/`, then `echoctl plugin update <id>` (asking for something new: the user's yes, `--yes`), then
`echoctl widget show …` again. Removing: `echoctl plugin remove <id>`; the folder stays until the user deletes it.
