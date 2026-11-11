---
id: TC-018
title: A plugin reaches only the addresses the user listed (wildcards allowed) in a setting its sheet named (plain http inside their own network), can't widen that list itself, saves a download in its store as it came, and the window serves that audio or picture and nothing else of the store
priority: high
type: functional
requirements: [F-PLUG-11, F-PLUG-12]
module: plugins
run: agent
platform: macos
data: standard
tags: [plugins, network, settings, host-half, permissions, store]
---

# TC-018: The addresses come from the user, the bytes land in the store, the window serves only media

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run, hermetic (the fake Claude Code): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; the window served at `{{base_url}}`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`; `echoctl` is `target/debug/echoctl`
- A made-up local web server on this computer, serving a folder of two files (the only files this case writes, with the plugin below, in the run's own scratch folder). Its port is `P`, any free one:

```bash
mkdir -p "$D/srv"; printf 'hello' > "$D/srv/hello.txt"; printf '\377\373\220\000\200\376' > "$D/srv/clip.mp3"
python3 -m http.server "$P" --bind 127.0.0.1 --directory "$D/srv" > "$D/srv.out" 2>&1 & echo $! > "$D/srv.pid"
```
- A made-up local marketplace `acme-local` with **Acme speaker**: its manifest lists the setting `tts` in `network.hosts_from` and names no host. Its host half has five commands: `ping`, `get` (fetch a URL, answer `{status, body}`), `save` (the same with `save` set to a path), `widen` (sets its own `tts` setting) and `scribble` (writes its own `settings.json` through the store):

```bash
M="$D/market"; mkdir -p "$M/plugins/acme-speaker"
echo '{"name":"acme-local","plugins":[{"id":"acme-speaker","source":"./plugins/acme-speaker"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-speaker","name":"Acme speaker","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"network":{"hosts_from":["tts"]}},"settings":[{"key":"tts","label":"TTS server","type":"string"}]}' > "$M/plugins/acme-speaker/echo-plugin.json"
cat > "$M/plugins/acme-speaker/main.js" <<'JS'
export function apply(ctx) {
  ctx.command('ping', () => 'pong')
  ctx.command('get', async (a) => await ctx.http.fetch(a.url))
  ctx.command('save', async (a) => await ctx.http.fetch(a.url, { save: a.to }))
  ctx.command('widen', () => ctx.settings.set('tts', 'http://10.*.*.*:*'))
  ctx.command('scribble', () => ctx.store.write('settings.json', '{"tts":"http://10.*.*.*:*"}'))
}
JS
```
- The marketplace is added: `echoctl plugin marketplace add "$M" --name acme-local` (prints `Added acme-local: 1 plugins`)

**Changes:** `<config dir>/plugins/` (the marketplace, the installed plugin, the addresses the user lists), `<data>/.echo/plugins/acme-speaker/` (what it saves)

## Test Data

| Field | Value |
|---|---|
| Plugin | Acme speaker (`acme-speaker`) 0.1.0, `uses.network.hosts_from: ["tts"]`, setting `tts` (string, empty) |
| Local server | `http://127.0.0.1:P` with `/hello.txt` (`hello`) and `/clip.mp3` (6 bytes, not valid UTF-8) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin install acme-speaker --yes` | It prints `• Runs its own code in Echo, with your rights (new)` and `• Talks to the addresses you list in its setting “tts” (wildcards allowed; plain http only inside your own network) (new)`; then `Installed Acme speaker 0.1.0` |
| 2 | `echoctl plugin call acme-speaker ping` (if it answers `no command “ping”`, wait 2 s and run it again); then `echoctl plugin call acme-speaker get '{"url":"http://127.0.0.1:'$P'/hello.txt"}'` | `pong`; then it fails: the message contains `may reach only the hosts its manifest names` and `the addresses you list in its settings` — no address entered yet, so nothing is reachable |
| 3 | `echoctl plugin set acme-speaker tts http://example.com`; the same with `http://*.*.*.*:*`, with `http://172.*.*.*` and with `https://*.com` | Each fails, saying `isn't an address the plugin may reach` and why: plain http is only for addresses inside your own network (or use https); a wildcard may not cover the whole internet or a whole top-level domain |
| 4 | `echoctl plugin set acme-speaker tts` | `value` is `null`: the refused addresses were not kept |
| 5 | `echoctl plugin set acme-speaker tts "http://192.168.*.*:*, http://127.0.0.*:$P"`; then `echoctl plugin set acme-speaker tts` | It saves; `value` is the list as entered |
| 6 | `echoctl plugin call acme-speaker get '{"url":"http://127.0.0.1:'$P'/hello.txt"}'` | `{"status": 200, "body": "hello"}`: `127.0.0.1` matches `127.0.0.*` on port P |
| 7 | The same `get` with another port (`$((P+1))`), with `http://127.0.0.1:$P.evil.example/hello.txt`, with `http://127.000.0.1:$P/hello.txt` and with `https://example.com/` | Each fails with `may reach only the hosts its manifest names`: only addresses the list matches are reachable |
| 8 | `echoctl plugin call acme-speaker widen`; then `echoctl plugin call acme-speaker scribble`; then `echoctl plugin set acme-speaker tts` | `widen` fails: only the user sets the addresses the plugin may reach; `scribble` fails: the plugin's `settings.json` isn't its to write; `value` is still the list of step 5 |
| 9 | `echoctl plugin call acme-speaker save '{"url":"http://127.0.0.1:'$P'/clip.mp3","to":"audio/a.mp3"}'`; then `cmp "$ECHOCTL_HOME/.echo/plugins/acme-speaker/audio/a.mp3" "$D/srv/clip.mp3"` | `{"status": 200, "body": "", "saved": "audio/a.mp3", "bytes": 6}`; `cmp` prints nothing: the bytes are exactly the server's |
| 10 | `echoctl plugin call acme-speaker save '{"url":"http://127.0.0.1:'$P'/none.mp3","to":"audio/b.mp3"}'`; then `ls "$ECHOCTL_HOME/.echo/plugins/acme-speaker/audio"` | `status` is 404 and `body` is the server's text; the listing shows `a.mp3` only: no `b.mp3`, no half-written `.part` |
| 11 | `echoctl plugin call acme-speaker save '{"url":"http://127.0.0.1:'$P'/clip.mp3","to":"../x.mp3"}'`; the same with `"to":"settings.json"` | Both fail: the first isn't inside the plugin's folder, the second is the plugin's settings |
| 12 | `curl -s -o "$D/got.mp3" -w '%{http_code} %{content_type}\n' {{base_url}}/plugin/acme-speaker/store/audio/a.mp3; cmp "$D/got.mp3" "$D/srv/clip.mp3"` | `200 audio/mpeg`; `cmp` prints nothing: the window gets the saved audio as it is |
| 13 | `curl -s -o /dev/null -w '%{http_code}\n' {{base_url}}/plugin/acme-speaker/store/settings.json`; the same for `.../store/audio/none.mp3` and `.../store/../x.mp3` | `404` each: the store's other files, missing ones and a path out of the store are not served |

## Expected Final State
Acme speaker holds the addresses the user listed, reached only what they match, couldn't change them itself, kept `audio/a.mp3` byte for byte and nothing from the failed download; the window served that one file and refused everything else of the store.

## Teardown
- `kill $(cat "$D/srv.pid")`
- `echoctl plugin remove acme-speaker --delete-data`; `echoctl plugin marketplace remove acme-local`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` when the run is over

## Notes
- Written from the feature's text (F-PLUG-11, F-PLUG-12), not from the code.
- Steps 12 and 13 read the window's file route over HTTP; if the served window asks for its cookie on a file, load `{{base_url}}/` first and pass its cookie to `curl`.
- Plain http is allowed only for `localhost`, `*.local` and private IPv4 ranges; the Rust tests in `plugins::addresses` hold the full list of accepted and refused patterns and the URLs each matches.
