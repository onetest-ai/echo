---
id: TC-022
title: Echo's MCP server offers only the search tools — search_sources finds what echoctl brain search finds, read_source and read_link read around it, the people and relations tools are gone
priority: high
type: functional
requirements: [F-KNOW-05, F-KNOW-07]
module: cli
run: agent
platform: any
data: standard
tags: [cli, recall, search, mcp]
---

# TC-022: Echo's MCP server offers only the search tools

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --set standard`; the shell has run `. <dir>/env`, and
  `ECHOCTL_CONFIG_DIR` points into `<dir>` before every command; no embedding server is set (the test config has none)
- `mcp() { printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' "$1" | target/debug/echoctl brain mcp | tail -1; }`
  (one request after `initialize`; the last line is its answer)

**Changes:** `<config dir>/brain.json` (`enabled`), `<home>/recall/knowledge.sqlite` (the index of the test calls)

## Test Data

| Field | Value |
|-------|-------|
| Query | cap yearly increase |
| Its call | Acme renewal negotiation (minutes: "Acme wants a cap on the yearly increase") |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl brain set --enabled true`; `target/debug/echoctl brain sync` | Both exit 0; the sync's JSON has `added` above 0 and no `pages_read` or `pages_pending` |
| 2 | `mcp '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'` | `result.tools[].name` is exactly `search_sources`, `list_sources`, `read_source`, `read_link` |
| 3 | For each of `find_people`, `get_person`, `find_related`, `get_evidence`, `list_scopes`: `mcp '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"<it>","arguments":{}}}'` | Each answers `result.isError` true with `no tool` in its text |
| 4 | `mcp '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"search_sources","arguments":{"query":"cap yearly increase","limit":3}}}'`; then `target/debug/echoctl brain search "cap yearly increase" --limit 3` | The tool's `result.structuredContent.hits[0].breadcrumb_path` starts with "Acme renewal negotiation", the same first hit (`chunk_id`) as the CLI's `hits[0]`; its `link` is an `echo://call/…` link |
| 5 | `mcp '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"read_source","arguments":{"source":"<step 4's hits[0].source>","titles_only":true}}}'` | `structuredContent.sections` lists the call's sections in order, one of them step 4's `chunk_id` |
| 6 | `mcp '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"read_link","arguments":{"link":"<step 4's hits[0].link>"}}}'` | `structuredContent.text` holds the call's minutes, with "cap on the yearly increase" |
| 7 | `mcp '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"search_knowledge","arguments":{"query":"cap yearly increase"}}}'` | The old name still answers, as search_sources (`isError` false, the same first hit) |
| 8 | Read step 2's `initialize` answer: `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \| target/debug/echoctl brain mcp` | `result.instructions` names search_sources, read_source and read_link and says people and relations are the Knowledge plugin's; it names none of `find_people`, `get_person`, `list_scopes` |
| 9 | `target/debug/echoctl brain related 1`; `target/debug/echoctl brain mentions --thread acme-rollout --title x` | Both exit non-zero: no such command |
| 10 | `target/debug/echoctl brain set --enabled false` | Exit 0 |

## Expected Final State
Intake is off again; the index of the test calls stays on disk.

## Notes
- The session block and the window's view hints name the same tools: Rust tests `brain::advert::tests` and
  `view::tests::with_intake_on_the_hint_names_search_sources_and_leaves_people_to_knowledge`.
- Search's own ranking (common words, header names): `cli/TC-021`.
