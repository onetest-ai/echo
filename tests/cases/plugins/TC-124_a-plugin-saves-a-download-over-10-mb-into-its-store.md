---
id: TC-124
title: A plugin saves a download over 10 MB into its store; without saving it is refused into memory
priority: medium
type: functional
requirements: [F-PLUG-83]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, network, files, cli]
---

# TC-124: A plugin downloads a big file into its store

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- TC-120's preconditions, run as written (in S-39: kept from that stage), every Bash call with the prelude `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; export D`
- A made-up file server on 48123 with a 12 MB file, and its address listed:

```bash
mkdir -p "$D/files"; head -c 12582912 /dev/zero > "$D/files/big.bin"
nohup python3 -m http.server 48123 --bind 127.0.0.1 --directory "$D/files" > "$D/files.out" 2>&1 & echo $! > "$D/files.pid"
echoctl plugin set acme-relay voice "http://127.0.0.1:48121, http://127.0.0.1:48123"
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48123/big.bin"}'` | It fails (`127.0.0.1:48123 can't be reached: curl: (63) …`, the 10 MB an answer in memory may be) |
| 2 | `echoctl plugin call acme-relay get '{"url":"http://127.0.0.1:48123/big.bin","opts":{"save":"cache/big.bin"}}'`; then `wc -c < "$ECHOCTL_HOME/.echo/plugins/acme-relay/cache/big.bin"` | `{"body": "", "bytes": 12582912, "saved": "cache/big.bin", "status": 200}`; `12582912` |

## Expected Final State
Acme relay's store holds `cache/big.bin` (12 MB).

## Teardown
- `kill $(cat "$D/files.pid")`; `echoctl plugin set acme-relay voice "http://127.0.0.1:48121"`. On its own: TC-120's teardown.

## Notes
- Written from the feature's text (F-PLUG-83). Sending a file: TC-128 (where it arrives) and TC-129 (what is refused).
- The store's place: `<home>/.echo/plugins/<id>/` (Host § Store).
