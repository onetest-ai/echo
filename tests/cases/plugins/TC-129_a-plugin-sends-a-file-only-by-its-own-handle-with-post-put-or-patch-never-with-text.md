---
id: TC-129
title: A plugin sends a file as a body only by a handle the user gave it, only with POST, PUT or PATCH, never with text as well and never with a length of its own
priority: medium
type: functional
requirements: [F-PLUG-83]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, files, cli]
---

# TC-129: What a plugin's file upload refuses

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-39: kept from that stage): Acme relay installed, the voice server on 48121, every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- Its address listed: `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"`; a made-up file: `printf 'acme note' > "$D/note.txt"`; the uploads the server heard so far: `grep -c "^post\|^put" "$D/ws.log"` (note the number)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48121/up","opts":{"method":"GET","file":"x"}}'` | It fails with `a file goes with POST, PUT or PATCH, not GET` |
| 2 | `echoctl plugin call acme-relay get --file "$D/note.txt" '{"url":"http://127.0.0.1:48121/up","opts":{"method":"POST","file":"x"}}'` | It fails with `“x” isn't a file handed to acme-relay`: only a handle of its own is sent, never a name it makes up |
| 3 | `echoctl plugin call acme-relay get --file "$D/note.txt" '{"url":"http://127.0.0.1:48121/up","opts":{"method":"PUT","body":"a","file":"x"}}'` | It fails with `a request sends a body or a file, not both` |
| 4 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48121/up","opts":{"method":"POST","file":"x","headers":{"Content-Length":"1"}}}'`; then `grep -c "^post\|^put" "$D/ws.log"` | It fails with `a file's Content-Length and Transfer-Encoding Echo sets itself`; the number noted: none of these reached the server |

## Expected Final State
Unchanged.

## Teardown
- `rm "$D/note.txt"`. On its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-83). The `get` command passes its `opts` to `ctx.http.fetch` as given, so a handle the plugin didn't get is refused (step 2); a file over 512 MB is refused by the same check (not run: too big for a case).
