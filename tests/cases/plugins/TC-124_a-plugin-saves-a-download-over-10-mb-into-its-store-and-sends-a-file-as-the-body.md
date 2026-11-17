---
id: TC-124
title: A plugin saves a download over 10 MB into its store (refused into memory) and sends a file the user handed it as a request's body
priority: medium
type: functional
requirements: [F-PLUG-83]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, files, cli]
---

# TC-124: A plugin downloads a big file into its store and uploads a file the user handed it

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-12: kept from that stage), every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- A made-up file server on 48123 with a 12 MB file, and its address listed:

```bash
mkdir -p "$D/files"; head -c 12582912 /dev/zero > "$D/files/big.bin"; printf 'acme note' > "$D/note.txt"
nohup python3 -m http.server 48123 --bind 127.0.0.1 --directory "$D/files" > "$D/files.out" 2>&1 & echo $! > "$D/files.pid"
echoctl plugin set acme-relay voice "http://127.0.0.1:48121, http://127.0.0.1:48123"
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48123/big.bin"}'` | It fails (`127.0.0.1:48123 can't be reached: curl: (63) …`, the 10 MB an answer in memory may be) |
| 2 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48123/big.bin","opts":{"save":"cache/big.bin"}}'`; then `wc -c < "$ECHOCTL_HOME/.echo/plugins/acme-relay/cache/big.bin"` | `{"body": "", "bytes": 12582912, "saved": "cache/big.bin", "status": 200}`; `12582912` |
| 3 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48123/up","opts":{"method":"GET","file":"x"}}'` | It fails with `a file goes with POST, PUT or PATCH, not GET` |
| 4 | `echoctl plugin call acme-relay get --file "$D/note.txt" '{"url":"http://127.0.0.1:48121/ping","opts":{"method":"POST","file":"x"}}'` | It fails with `“x” isn't a file handed to acme-relay`: only a handle of its own is sent, never a name it makes up |
| 5 | `echoctl plugin call acme-relay get --file "$D/note.txt" '{"url":"http://127.0.0.1:48123/up","opts":{"method":"PUT","body":"a","file":"x"}}'` | It fails with `a request sends a body or a file, not both` |

## Expected Final State
Acme relay's store holds `cache/big.bin` (12 MB).

## Teardown
- `kill $(cat "$D/files.pid")`. In S-12 kept for TC-125; on its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-83). The `get` command passes its `opts` to `ctx.http.fetch` as given, so a handle the plugin didn't get is refused (step 4); the file's bytes arriving at a server whole (size and sha256) are proven by the Rust test `jshost::net::tests::a_plugin_streams_an_answer_as_it_comes_cancels_it_and_sends_and_saves_big_files` — the fake servers here take no uploads.
- The store's place: `<home>/.echo/plugins/<id>/` (Host § Store).
