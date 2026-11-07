---
id: TC-013
title: echoctl read gives the text behind a call (around a moment), thread, client, person, task and prep link, and refuses what isn't one
priority: medium
type: functional
requirements: [F-CLI-09]
module: cli
run: agent
platform: any
data: standard
tags: [cli, links, read]
---

# TC-013: echoctl read gives the text behind a call (around a moment), thread, client, person, task and prep link, and refuses what isn't one

**Module:** CLI | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)` (today's Acme weekly sync)

## Test Data

| Link | Behind it |
|---|---|
| `echo://call/$A?t=40` | Acme weekly sync, its turns within 90 s of 00:40 and its minutes |
| `echo://thread/acme-rollout` | Acme rollout: client, stage, brief, people, calls |
| `echo://client/acme` | Acme: its 3 threads and About |
| `echo://person/dana@acme.example` | Dana Lee's card |
| `echo://task/acme-rollout/T001` | Send the SSO checklist (done) |
| `echo://prep/e11de173ab857a8b` | the prep of Globex sponsor update |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl read "echo://call/$A?t=40"` | Exit code 0; Markdown starting "# Call: Acme weekly sync — <today> <HH:MM>"; a section "## Said around 00:40 (±90 s)" whose lines start `[00:05] Me:`, `[00:17] Dana Lee:`, `[00:29] Omar Haddad:`, `[00:43] Dana Lee:` … up to `[02:03] Omar Haddad:`; then "## Minutes" with "# Acme weekly sync" and the sections Summary, Decisions, Action items, Open questions |
| 2 | Run `target/debug/echoctl read "echo://call/$A"` (no moment) | No "Said around" section; "## Minutes" follows the title line |
| 3 | Run `target/debug/echoctl read echo://thread/acme-rollout` | Starts "# Thread Acme rollout — client Acme (echo://client/acme), stage Active"; sections "## What it is" (the brief: "Rolling out **single sign-on** and the data migration for Acme, Berlin first."), "## People" (first line "- [Dana Lee](echo://person/dana@acme.example) (Acme): delivery lead") and "## Calls" with 6 lines, each an `echo://call/…` link |
| 4 | Run `target/debug/echoctl read echo://client/acme` | Starts "# Acme — client, acme.example"; "## Threads" lists Acme hackathon — Cancelled, Acme renewal — Negotiation, Acme rollout — Active, each with its `echo://thread/…` link; then "## About" |
| 5 | Run `target/debug/echoctl read echo://person/dana@acme.example` | Starts "# Dana Lee", then "Delivery manager · Acme · dana@acme.example"; "## Threads" with "- Acme rollout: delivery lead"; "## What was said about them" |
| 6 | Run `target/debug/echoctl read echo://task/acme-rollout/T001` | Starts "# Send the SSO checklist — task (done)", then "Thread: echo://thread/acme-rollout", "Owner: Me"; "## Where it was said" with a `create` line quoting "I will send the SSO checklist to Dana by Friday" |
| 7 | Run `target/debug/echoctl read echo://prep/e11de173ab857a8b` | Starts "# Prep"; "In short: Globex sponsor update: the SSO checklist is done, the questionnaire waits for legal, the access list is late."; "Meeting: Globex sponsor update (…)" |
| 8 | Run `target/debug/echoctl read echo://thread/no-such-thread` | Exit code 1; "Error: no thread \"no-such-thread\"" |
| 9 | Run `target/debug/echoctl read https://acme.example` and `target/debug/echoctl read "echo://call/../../etc"` | Exit code 1 each; "Error: not an echo:// link Echo knows: https://acme.example" and "Error: not an echo:// link Echo knows: echo://call/../../etc" |

## Expected Final State
Read only: nothing in the data folder changed.

## Notes
- The prep id is derived from the meeting's event id, so it is the same on every build of the standard set; the window opens the same links in `shell/TC-016`.
- Mail links (`echo://mail/…`) need the Brain with Microsoft 365 mail: not on test data.
