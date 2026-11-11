---
id: TC-016
title: A plugin hears only the Echo events it named in uses.events.on — the sheet says which, ctx.on accepts those and refuses every other with "didn't ask to hear"
priority: high
type: functional
requirements: [F-PLUG-08]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, events, permissions, host-half]
---

# TC-016: A plugin hears only the events it named

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); CLI steps run in Bash after `. <dir>/env`; the browser 1201 × 868
- A made-up local marketplace `acme-local` with **Acme listener**: a host half that asks to hear three events, though its manifest names only one. The only files this case writes, in the run's own scratch folder, after `. <dir>/env`:

```bash
M="$(dirname "$ECHOCTL_HOME")/market"; mkdir -p "$M/plugins/acme-listener"
echo '{"name":"acme-local","plugins":[{"id":"acme-listener","source":"./plugins/acme-listener"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-listener","name":"Acme listener","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"events":{"on":["minutes_written"]}}}' > "$M/plugins/acme-listener/echo-plugin.json"
cat > "$M/plugins/acme-listener/main.js" <<'EOF'
export function apply(ctx) {
  const heard = []
  const accepted = []
  const refused = {}
  for (const name of ['minutes_written', 'call_started', 'recording_started']) {
    ctx.on(name, (p) => { heard.push(name + ':' + p.call) })
      .then(() => { accepted.push(name) })
      .catch((e) => { refused[name] = String((e && e.message) || e) })
  }
  ctx.command('heard', () => ({ heard, accepted, refused }))
}
EOF
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-listener --yes` | It prints `• Runs its own code in Echo, with your rights (new)` and `• Knows when a call's minutes are written (the call's title) (new)` — and no line about a Teams call or a recording; then `Installed Acme listener 0.1.0` |
| 2 | `echoctl plugin call acme-listener heard` (if it answers `no command “heard”`, wait 2 s and run it again) | JSON with `"heard": []` (nothing happened in Echo), `"accepted": ["minutes_written"]`, and `"refused"` holding exactly two keys, `call_started` and `recording_started`; each message contains `acme-listener didn't ask to hear` followed by that event's name in “ ” and `when it was installed (uses.events.on)` |
| 3 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#settings/plugins`, click the row "Acme listener" | The detail's "What it can do" lists "Runs its own code in Echo, with your rights" and "Knows when a call's minutes are written (the call's title)"; it lists nothing about a Teams call starting or a recording starting |
| 4 | Change what the plugin asks for on the marketplace side: `echo '{"id":"acme-listener","name":"Acme listener","version":"0.2.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"events":{"on":["minutes_written","recording_started"]}}}' > "$M/plugins/acme-listener/echo-plugin.json"`; `echoctl plugin marketplace update acme-local`; `echoctl plugin update acme-listener --yes` | It prints `• Knows when a recording starts (the call's title) and a call's minutes are written (the call's title) (new)` and no line about a Teams call; then `Updated Acme listener to 0.2.0` |
| 5 | `echoctl plugin call acme-listener heard` (as in step 2) | `"accepted"` holds `minutes_written` and `recording_started`; `"refused"` has the one key `call_started` (still not asked for), with the same wording as step 2 |

## Expected Final State
Acme listener at 0.2.0 hears the two events its manifest names; the one it never named is still refused.

## Teardown
- `echoctl plugin remove acme-listener --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; close the test tab

## Notes
- `ctx.on(event, fn)` asks `events.on` of the host, which checks `plugins::heard_events` (the plugin's `uses.events.on`, kept as it loaded); an event not named is refused and its handler is dropped, so it is never delivered. The refusal is the observable half here.
- An event itself (a payload of ids and titles, never the call's words) needs something to happen in Echo (minutes written, a recording); a terminal can't make one. That half is the Rust test `jshost::a_plugin_hears_only_the_events_it_asked_for`, and a live check is `run: person` territory.
