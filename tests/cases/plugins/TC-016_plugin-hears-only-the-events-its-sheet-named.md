---
id: TC-016
title: A plugin hears only the Echo events it named in uses.events.on — the sheet says which, ctx.on accepts those and refuses every other with "didn't ask to hear"; a recording it named reaches it with the call's id and title, never the words
priority: high
type: functional
requirements: [F-PLUG-08]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, events, permissions, host-half, recording, file-fed, second-instance]
---

# TC-016: A plugin hears only the events it named, with ids and titles and never the words

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic (the fake Claude Code): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first (its cookie lets clicks reach the app); the browser 1201 × 868
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; W=tests/fixtures/wait.py; export D`; `echoctl` is `target/debug/echoctl`
- Calls are fed from files, as in `calls/TC-040` (app_profile.md → "Recording with file-fed calls"): no device, no sound, no real account. **Needs a second app instance** (the plugin hears events inside the running app); if `$D/app.out` says "already running" the case is BLOCKED, not failed
- A made-up local marketplace `acme-local` with **Acme listener**: a host half that asks to hear three events, though its manifest names only one. It keeps every event it hears in its own store and answers `heard` with them. The only files this case writes, in the run's own scratch folder, after the prelude:

```bash
M="$D/market"; mkdir -p "$M/plugins/acme-listener"
echo '{"name":"acme-local","plugins":[{"id":"acme-listener","source":"./plugins/acme-listener"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-listener","name":"Acme listener","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"events":{"on":["minutes_written"]}}}' > "$M/plugins/acme-listener/echo-plugin.json"
cat > "$M/plugins/acme-listener/main.js" <<'JS'
export function apply(ctx) {
  const got = []
  const accepted = []
  const refused = {}
  for (const name of ['minutes_written', 'call_started', 'recording_started']) {
    ctx.on(name, (p) => { got.push({ event: name, payload: p }); ctx.store.write('heard.json', JSON.stringify(got)) })
      .then(() => { accepted.push(name) })
      .catch((e) => { refused[name] = String((e && e.message) || e) })
  }
  ctx.command('heard', () => {
    let heard = []
    try { heard = JSON.parse(ctx.store.read('heard.json')) } catch (e) {}
    return { heard, accepted, refused }
  })
}
JS
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, its store), a recording folder under `calls/`

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme listener (`acme-listener`), 0.1.0 naming `minutes_written`, then 0.2.0 naming `minutes_written` and `recording_started` |
| Conversation | acme-sync (36 s), fed from files, held |
| Arm title | REC-016 listener (the folder ends `-rec-016-listener`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-listener --yes` | It prints `• Runs its own code in Echo, with your rights (new)` and `• Knows when a call's minutes are written (the call's title) (new)` — and no line about a Teams call or a recording; then `Installed Acme listener 0.1.0` |
| 2 | `echoctl plugin call acme-listener heard` (if it answers `no command “heard”`, wait 2 s and run it again) | JSON with `"heard": []` (nothing happened in Echo), `"accepted": ["minutes_written"]`, and `"refused"` holding exactly two keys, `call_started` and `recording_started`; each message contains `acme-listener didn't ask to hear` followed by that event's name in “ ” and `when it was installed (uses.events.on)` |
| 3 | Reload `{{base_url}}/`, navigate to `{{base_url}}/#settings/plugins`, click the row "Acme listener" | The detail's "What it can do" lists "Runs its own code in Echo, with your rights" and "Knows when a call's minutes are written (the call's title)"; it lists nothing about a Teams call starting or a recording starting |
| 4 | Change what the plugin asks for on the marketplace side: `echo '{"id":"acme-listener","name":"Acme listener","version":"0.2.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"events":{"on":["minutes_written","recording_started"]}}}' > "$M/plugins/acme-listener/echo-plugin.json"`; `echoctl plugin marketplace update acme-local`; `echoctl plugin update acme-listener --yes` | It prints `• Knows when a recording starts (the call's title) and a call's minutes are written (the call's title) (new)` and no line about a Teams call; then `Updated Acme listener to 0.2.0` |
| 5 | `echoctl plugin call acme-listener heard` (as in step 2) | `"accepted"` holds `minutes_written` and `recording_started`; `"refused"` has the one key `call_started` (still not asked for), with the same wording as step 2; `"heard": []` |
| 6 | `echoctl config --languages en --unscheduled manual --scheduled manual --auto-minutes false`; `echoctl ai live off`; `nohup echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"; $W 30 app` | The config calls exit 0; "waited … for app" |
| 7 | `echoctl arm --title "REC-016 listener"; $W 10 armed; $C start acme-sync --hold; $W 30 recording rec-016` | "waited … for recording rec-016": the call started and the recording with it (Echo itself has now fired `call_started` and `recording_started`) |
| 8 | `echoctl plugin call acme-listener heard` (wait 2 s and run it again if `"heard"` is still empty) | `"heard"` holds exactly one entry, for `recording_started`: its payload names the call by its id (a name ending `-rec-016-listener`) and its title `REC-016 listener`; every value in the payload is that id, that title or a true/false flag, and none is a sentence said in the call. There is no entry for `call_started` |
| 9 | `$C end; target/debug/echoctl stop; $W 60 event recording_stopped`; then `echoctl plugin call acme-listener heard` | `"heard"` still holds that one entry: the hang-up and the stop, which Echo also announced, reached the plugin as nothing, because it never named them |

## Expected Final State
Acme listener at 0.2.0 hears the two events its manifest names; the one it never named is refused; of everything the recording fired it was given only `recording_started`, with the call's id and title and none of the words.

## Teardown
- `$C end` (ignore "no call"); `target/debug/echoctl app quit` (if it still runs after 30 s: `kill $(cat "$D/app.pid")`)
- `echoctl plugin remove acme-listener --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; close the test tab; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-08: "with ids and titles, never the words"), not from the code; a payload with the call's text in it is a FAIL.
- The first half (steps 1–5) needs no app running: the refusals are what `ctx.on` answers when the event was not named. The second half (steps 6–9) is the delivery, with a recording made from files as in `calls/TC-040`.
- The transcript and minutes events (`transcribed`, `minutes_written`) are the ones that could carry words; they need a speech model and a model run, so this case reads the recording event, which is fired while the words are not yet written. A person-run check of those two is open.
