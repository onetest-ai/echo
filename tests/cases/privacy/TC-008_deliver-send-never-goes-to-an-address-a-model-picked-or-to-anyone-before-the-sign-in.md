---
id: TC-008
title: Sending with deliver is refused before a sign-in, never offered to a run's model, and an address given for Today is refused, never sent or kept
priority: high
type: functional
requirements: [F-M365-04]
module: privacy
run: agent
platform: any
data: small
tags: [privacy, deliver, guard, m365]
---

# TC-008: Sending with deliver is refused before a sign-in, never offered to a run's model, and an address given for Today is refused

**Module:** Privacy | **Priority:** High | **Type:** Functional

## Preconditions
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. Microsoft 365 is not signed in on test data (no `<dir>/config/graph.json`), no plugin installed
- `CALL=$(ls -d $ECHOCTL_HOME/calls/*-acme-weekly-sync)`; a text file `<dir>/m.md` holding the line `Test text, nothing real.`
- What this case covers (F-M365-04: sends go only to the user's own Teams chat and mailbox, or a plugin's way): nothing is sent before the sign-in; a model's run never gets `deliver`; no address is carried to Today, the user's own page. The send itself to a real Teams chat and mailbox needs a sign-in: see Notes

## Test Data

| Field | Value |
|---|---|
| Address a model might pick | `ana@globex.example` (never sent to; used only to show it is not carried anywhere) |
| Text file | `<dir>/m.md` |
| Probe | the run guard (`echoctl run-guard`, Claude Code's hook) fed one tool call on stdin, with the environment of the run's mode |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl deliver status` | JSON with `"teams": false`, `"mail": false`, `"today": true`, `"account": null`, `"provided": []` |
| 2 | Run `target/debug/echoctl deliver send mail --title "Probe" --file <dir>/m.md; echo "exit $?"` | stderr "Error: not signed in to Microsoft 365 (Settings → Calendar & chat)", then "exit 1"; no "Sent" |
| 3 | Run `target/debug/echoctl deliver send teams --title "Probe" --file <dir>/m.md; echo "exit $?"` and `target/debug/echoctl deliver test mail; echo "exit $?"` | Each prints "Error: not signed in to Microsoft 365 (Settings → Calendar & chat)" and "exit 1" |
| 4 | Run `target/debug/echoctl deliver send telegram --title "Probe" --file <dir>/m.md; echo "exit $?"` | "Error: “telegram” isn't a way Echo sends (its plugin isn't installed and on)", "exit 1" |
| 5 | Run `printf '{"tool_name":"Bash","tool_input":{"command":"echoctl deliver send mail --title t --file x.md"}}' \| ECHOCTL_RUN_CALL="$CALL" ECHOCTL_RUN_CLIS=echoctl target/debug/echoctl run-guard; echo "exit $?"` | "Echo's minutes run: echoctl deliver isn't one of the commands a minutes run uses", "exit 2" |
| 6 | Run the same probe with `ECHOCTL_RUN_MODE=routine ECHOCTL_RUN_HOME="$ECHOCTL_HOME" ECHOCTL_RUN_CLIS=echoctl` in place of `ECHOCTL_RUN_CALL=…` | "Echo (this routine reads only): echoctl deliver isn't one of the commands a read-only routine uses", "exit 2" |
| 7 | Run the same probe with `ECHOCTL_RUN_MODE=ask ECHOCTL_RUN_CALL="$CALL" ECHOCTL_RUN_CLIS=echoctl` and the command `echoctl deliver test mail` | "Echo (this call's Claude session reads only): echoctl deliver isn't one of the commands this session uses", "exit 2" |
| 8 | For comparison run the first probe with the command `echoctl context <the value of $CALL>` | No output, "exit 0" (the guard lets a run's own commands through) |
| 9 | Run `printf '## Hi\n\nText.\n' \| target/debug/echoctl deliver send today --title "Own note" --file /dev/stdin --to ana@globex.example; echo "exit $?"` and `cat $ECHOCTL_HOME/.echo/today-cards.json` | The command is refused (`deliver send` takes no `--to`: it sends to you only), exit not 0; nothing sent; `today-cards.json` has no "Own note" card and no address |
| 10 | Run `printf '## Hi\n\nText.\n' \| target/debug/echoctl deliver send today --title "Own note" --file /dev/stdin` and `cat $ECHOCTL_HOME/.echo/today-cards.json` | "Sent"; the file holds one card, `"title": "Own note"`, `"seen": false` (the user's own Today still takes a send) |

## Expected Final State
No send left the computer: Teams and Mail refused with the sign-in message, every run mode refused `deliver`, and an address given with Today was refused, and the one thing sent, a Today card, was sent without one.

## Teardown
- Remove `$(dirname <dir>)`; nothing outside the run's folder was touched.

## Notes
- `deliver send` has no `--to` (#401): it sends to you only. Mail to others goes through `echoctl m365 mail send`, which sends only after the user's yes; a routine's addresses go through its plugin, as the user set them.
- Not shown here, needs the real sign-in (person, with the person's own test account and mailbox, nothing sent to anyone else): signed in, `echoctl deliver test teams` lands "Echo test" in the account's own Teams chat with itself; `echoctl deliver test mail` lands one mail "Echo test" in the account's own Inbox and not in Sent. Settings → Sends to you → "Send a test" does the same from the window (`settings/TC-020` shows the signed-out rows).
- Step 4's wording is for the build with plugins (`js`, the default); a build without them says "“telegram” isn't a way Echo sends".
