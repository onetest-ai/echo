---
id: TC-105
title: A plugin fetches a web address the user typed in Echo's own box — that one only, once, never where it redirects; text that isn't an address, a cancelled box and a run Echo started give it nothing
priority: high
type: functional
requirements: [F-PLUG-73]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, network, typed-address]
---

# TC-105: A plugin fetches the address the user typed, once

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl` (a debug build: `ECHOCTL_ASK_ADDRESS` answers Echo's box instead of a dialog — a release build never reads it, so a step never runs without it)
- Data demo installed: in S-36, stage 1. On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes` (its sheet says "Fetches a web page from an address you type in Echo's own box when it asks — that page only, once each time")
- A made-up web site on this computer, `D="$(dirname "$ECHOCTL_HOME")"`:

```bash
mkdir -p "$D/site/notes"
printf '<html><head><title>Acme pilot notes</title></head><body><h1>Acme pilot</h1><p>The Berlin office starts first.</p></body></html>\n' > "$D/site/pilot.html"
printf '<html><body><p>Acme notes index</p></body></html>\n' > "$D/site/notes/index.html"
(cd "$D/site" && nohup python3 -m http.server 48139 --bind 127.0.0.1 >/dev/null 2>&1 & echo $! > "$D/site.pid")
```

**Changes:** none (the site's folder is the run's)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call data-demo fetch '{"url":"http://127.0.0.1:48139/pilot.html"}'` | It fails: "data-demo may reach only an address you type in Echo's box when it asks, once" (nothing typed yet) |
| 2 | `ECHOCTL_ASK_ADDRESS="http://127.0.0.1:48139/pilot.html" echoctl plugin call data-demo fetch-typed` | `"address": "http://127.0.0.1:48139/pilot.html"`, `"status": 200`, `"body"` holds "Acme pilot", and `"again": "data-demo may reach only an address you type in Echo's box when it asks, once"` — the second fetch of the same address was refused |
| 3 | `ECHOCTL_ASK_ADDRESS="http://127.0.0.1:48139/notes" echoctl plugin call data-demo fetch-typed` | `"status": 301` (the site sends it to `/notes/`) and no "Acme notes index": where the page moved is no address the user typed, so it isn't followed |
| 4 | `ECHOCTL_ASK_ADDRESS="ftp://acme.example/x" echoctl plugin call data-demo fetch-typed` | It fails: "“ftp://acme.example/x” isn't a web address Echo fetches (http:// or https://, a host, no spaces)" |
| 5 | `ECHOCTL_ASK_ADDRESS= echoctl plugin call data-demo fetch-typed` (the box left empty, as a cancel) | `{"address": null}` |
| 6 | `ECHOCTL_HEADLESS=1 ECHOCTL_ASK_ADDRESS="http://127.0.0.1:48139/pilot.html" echoctl plugin call data-demo fetch-typed` | It fails: "Echo asks you for an address only in your own window or terminal, not inside a run Echo started" |

## Expected Final State
Nothing changed in Echo; the plugin keeps no address.

## Teardown
- `kill "$(cat "$D/site.pid")"`
- In S-36: keep the plugin. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The address lives in the asking process's memory only (`jshost::typed`): one per plugin, gone after its fetch or 10 minutes; another plugin, another address, a POST, headers or a body is refused — `jshost::a_plugin_fetches_the_address_the_user_typed_once_and_nothing_else`.
- On a person's Mac, the box is the system's dialog with Echo's words: "Data demo asks for the address of a web page to fetch once. Type or paste it:" (checked by hand at a release).
