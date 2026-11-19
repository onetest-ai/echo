---
id: TC-023
title: Recall search narrows by section, person, kind and a date said in the query, and keeps only the cards of old calls and of a closed thread's calls
priority: high
type: functional
requirements: [F-KNOW-10, F-KNOW-11]
module: cli
run: agent
platform: any
tags: [cli, recall, search, retention]
---

# TC-023: Recall search narrows by section, person, kind and a date said in the query, and keeps only the cards of old calls and of a closed thread's calls

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --set standard` today; the shell has run
  `set -a; . <dir>/env; set +a`, and `ECHOCTL_HOME` and `ECHOCTL_CONFIG_DIR` point into `<dir>` before every command
  (otherwise stop: the real data folder is never touched); the app is not running on it
- No embedding server is set (the test config has none)

## Test Data

| Field | Value |
|-------|-------|
| Calls older than 90 days | Initech all hands (40 of them, in no thread) |
| A closed thread's call | Tyrell bid review (thread Tyrell bid, lost) |
| A summary line | Acme renewal negotiation: "Acme wants a cap on the yearly increase" |
| A speaker | Dana Lee (Acme weekly sync's transcript) |
| Yesterday's calls | Acme security review, Vendor call with Vandelay |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl intake set --enabled true`, then `target/debug/echoctl intake sync` | Exit code 0 both; the sync's JSON has `added` plus `unchanged` above 0, the data may be synced already by an earlier case (what is past its horizon is never written, so `retired` is 0) |
| 1a | Run `target/debug/echoctl intake sync` again | Exit code 0; `added`, `changed`, `removed` and `chunks` are 0 and `unchanged` is step 1's `added` plus `unchanged`: every document (calls, mail, the channel post, the thread folder's file) counted unchanged, none written again |
| 2 | Run `target/debug/echoctl intake search "Initech all hands" --limit 5` | 5 hits, every one with `section_kind` "card" and `breadcrumb_path` "Initech all hands > Call card"; their `text` starts "Call: Initech all hands ·" and names its people |
| 3 | Run `target/debug/echoctl intake search "all hands news" --section transcript` | `count` is 0: the old calls' transcripts left the index |
| 4 | Run `ls <dir>/home/calls/*initech-all-hands-01/` | `segments.json` and `meta.json` are still there: only the index lost them |
| 5 | Run `target/debug/echoctl intake search "bid vendor" --thread tyrell-bid` | `count` is 1: Tyrell bid review's `Call card` (`section_kind` "card") with a `closed` list naming "Tyrell bid"; its `text` has "Summary: The bid went to another vendor" (its minutes and transcript left the index) |
| 6 | Run `target/debug/echoctl intake search "cap yearly increase" --section summary` | Every hit has `section_kind` "summary"; `hits[0].breadcrumb_path` is "Acme renewal negotiation > Minutes > Summary"; `filters.sections` is `["summary"]` |
| 7 | Run `target/debug/echoctl intake search "rollout" --section transcript --person Dana` | At least one hit; every hit has `section_kind` "transcript" and a `speaker` holding "Dana Lee" |
| 8 | Run `target/debug/echoctl intake search "rollout yesterday" --limit 5` | `filters.dates_from_query` is "yesterday"; `filters.since` and `filters.until` are both yesterday's date; every hit's `event_date` is that day |
| 8a | Run `target/debug/echoctl intake search "what happened yesterday"` | `note` starts "no section of those days has the words: the sources of those days, newest first"; at least 2 hits, each from a different `source`, every `event_date` yesterday's date, among them Acme security review and Vendor call with Vandelay |
| 9 | Run `target/debug/echoctl intake search "rollout" --kind mail,channel,document`, then the same with `--kind call --limit 3` | The first: `count` 0 and `filters.kinds` `["mail","channel","document"]`; the second: 3 hits, every `source` starting `call/` |
| 10 | Run `target/debug/echoctl intake search "rollout" --since <the day 60 days ago, YYYY-MM-DD>` | `note` ends with "older than the index keeps: see the call folders or Knowledge" |
| 11 | Run `target/debug/echoctl intake search rollout --kind fax`, then `target/debug/echoctl intake search rollout --since "12 Nov"` | Both exit non-zero: "no source kind \"fax\" (one of: call, mail, channel, document)", then "\"12 Nov\" isn't a day (YYYY-MM-DD)" |

## Expected Final State
Recall is on in the test config; its index keeps every call's card, the minutes of calls up to 90 days old, the
transcripts of calls up to 45 days old and nothing else of the closed thread's call; the call folders are unchanged.
