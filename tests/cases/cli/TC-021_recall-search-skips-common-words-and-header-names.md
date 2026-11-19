---
id: TC-021
title: Recall search finds a call from a question full of common words, and a name only in a Who line finds nothing
priority: high
type: functional
requirements: [F-KNOW-07]
module: cli
run: agent
platform: any
tags: [cli, recall, search]
---

# TC-021: Recall search finds a call from a question full of common words, and a name only in a Who line finds nothing

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --set standard`; the shell has run `. <dir>/env`, and
  `ECHOCTL_CONFIG_DIR` points into `<dir>` before every command; the app is not running on it
- No embedding server is set (the test config has none)

## Test Data

| Field | Value |
|-------|-------|
| Question of common words | what did they say about the cap on the yearly increase |
| Its call | Acme renewal negotiation (minutes: "Acme wants a cap on the yearly increase") |
| Name only in a Who line | Zora Quist (the minutes of Partner sync with Vandelay: `**Who:** [Zora Quist](echo://person/…), Sam Rivera (Me)`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl intake set --enabled true` | Exit code 0; JSON whose `settings.endpoint` is empty |
| 2 | Run `target/debug/echoctl intake sync` | Exit code 0; JSON with `added` above 0 and `embedded` 0 |
| 3 | Run `target/debug/echoctl intake search "what did they say about the cap on the yearly increase" --limit 3` | `note` starts with "keywords only"; `hits[0].breadcrumb_path` starts with "Acme renewal negotiation" and its `text` says "cap on the yearly increase" |
| 4 | Run `target/debug/echoctl intake search "Zora Quist" --limit 3` | `count` is 0: the name is only in the Who line, which isn't matched |
| 5 | Run `target/debug/echoctl intake search "Zora Quist identity work" --limit 3` | `hits[0].breadcrumb_path` is "Partner sync with Vandelay > Minutes > Summary" (found by "identity work"); no hit's `text` holds "Zora Quist" |
| 6 | Run `target/debug/echoctl intake search "who is this, and when?"` | `count` is 0 and `note` is "only common words; nothing to search" |

## Expected Final State
Recall is on in the test config and its index holds the test calls; search matched the questions' meaningful words
only, never a name that appears only in a minutes header line.
