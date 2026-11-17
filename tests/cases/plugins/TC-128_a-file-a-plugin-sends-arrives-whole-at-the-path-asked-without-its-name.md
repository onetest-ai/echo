---
id: TC-128
title: A file the user handed a plugin, sent as a request's body, arrives whole at the path asked — never with the file's name added — and in one piece, not in chunks
priority: medium
type: functional
requirements: [F-PLUG-83]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, files, cli]
---

# TC-128: A plugin uploads a file the user handed it to the user's server

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-39: kept from that stage): Acme relay installed (its `upload` command), the voice server on 48121 (it answers a POST or PUT with the body's size and logs `post <path> <bytes> <sha256>`), every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- Its address listed: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"`
- A made-up file with a name of its own: `head -c 300000 /dev/urandom > "$D/acme-clip.wav"; shasum -a 256 "$D/acme-clip.wav"` (note the hash)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay upload --file "$D/acme-clip.wav" '{"url":"http://127.0.0.1:48121"}'`; then `tail -1 "$D/ws.log"` | `{"body": "{\"ok\": true, \"bytes\": 300000, \"chunked\": false}", "status": 200}`; the log's line is `post / 300000 <the hash>`: the URL with no path arrives as `/`, the file whole |
| 2 | The same with `"url":"http://127.0.0.1:48121/api/"`; then `tail -1 "$D/ws.log"` | `post /api/ 300000 <the hash>` |
| 3 | The same with `"url":"http://127.0.0.1:48121/api"` and `"method":"PUT"`; then `tail -1 "$D/ws.log"`; then `grep -c "acme-clip" "$D/ws.log"` | `put /api 300000 <the hash>`; `0`: the file's name never reached the server |

## Expected Final State
Unchanged: Acme relay installed, its address listed.

## Teardown
- `rm "$D/acme-clip.wav"`. On its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-83). The curl config Echo writes for such a request (its headers) is gone once it ends: the Rust test `jshost::net::tests::a_plugin_streams_an_answer_as_it_comes_cancels_it_and_sends_and_saves_big_files`.
