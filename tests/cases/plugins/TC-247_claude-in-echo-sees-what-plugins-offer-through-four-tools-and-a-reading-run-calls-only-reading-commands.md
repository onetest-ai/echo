---
id: TC-247
title: Claude in Echo sees what plugins offer through the echo_plugins server's four tools, a level at a time, and a run that only reads calls only commands marked reads
priority: high
type: functional
requirements: [F-PLUG-158]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, offers, mcp, claude]
---

# TC-247: The echo_plugins server

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- As TC-245: the same data, prelude and the marketplace `acme-offers` (run its precondition lines when it isn't added yet); Acme vaults installed (`$E plugin install acme-vaults --yes` when it isn't)
- `mcp() { printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' "$1" | $E plugins-mcp $2 | tail -1; }`
  (one request after `initialize`; the last line is its answer; `$2` is `--read` for a run that only reads)

**Changes:** `<config dir>/plugins/` (`offers/`), Acme vaults' store

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'` | `result.tools[].name` is exactly `plugins`, `plugin`, `command`, `call` |
| 2 | `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"plugins","arguments":{}}}'`; then with `"name":"plugin","arguments":{"id":"acme-vaults"}` | The first's text lists `acme-vaults` with `Search and notes across the Acme vaults`; the second's its commands `search` and `note`, each with its description |
| 3 | `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"call","arguments":{"id":"acme-vaults","name":"search","args":{"query":"Acme"}}}}' --read` | `isError` false; its text `{"from":"claude","limit":10,"query":"Acme"}` |
| 4 | `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"call","arguments":{"id":"acme-vaults","name":"note","args":{"path":"a.md","text":"x"}}}}' --read`; then the same without `--read` | The first: `isError` true, `this run only reads` in its text; the second: `isError` false, text `"written a.md"` |
| 5 | `$E plugin remove acme-vaults`; `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"plugins","arguments":{}}}'` | Its text is `[]`: a removed plugin is offered to nobody |
