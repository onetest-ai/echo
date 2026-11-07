---
id: TC-016
title: echoctl m365 sends nothing without --yes: mail, a Teams message, a channel post and an upload answer what would be sent
priority: critical
type: functional
requirements: [F-M365-05]
module: cli
run: agent
platform: any
data: standard
tags: [cli, m365, privacy, sending]
---

# TC-016: echoctl m365 sends nothing without --yes: mail, a Teams message, a channel post and an upload answer what would be sent

**Module:** CLI | **Priority:** Critical | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Microsoft 365 is not signed in on this data folder (no `--yes` can reach anyone)
- The message is read from stdin (`--file /dev/stdin`), so the case writes no file: `printf 'Hello Dana,\n\nThe **checklist** is attached.\n' | target/debug/echoctl …`

## Test Data

| Field | Value |
|---|---|
| Text | `Hello Dana,\n\nThe **checklist** is attached.\n` |
| To | dana@acme.example (made-up), cc omar@acme.example |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `printf '…' \| target/debug/echoctl m365 mail send --to dana@acme.example --cc omar@acme.example --subject "SSO checklist" --file /dev/stdin` | Exit code 0; JSON `sent` false; `would.to[0].emailAddress.address` "dana@acme.example", `would.cc[0].emailAddress.address` "omar@acme.example", `would.subject` "SSO checklist", `would.text` the text exactly; `next` "Nothing was sent. Show the user exactly this; run the same command with --yes only after they agree." |
| 2 | Run `printf '…' \| target/debug/echoctl m365 teams send --to dana@acme.example --file /dev/stdin` | Exit code 0; `sent` false; `would` `{"kind":"oneOnOne","text":<the text>,"to":"dana@acme.example"}`; the same `next` |
| 3 | Run `printf '…' \| target/debug/echoctl m365 teams post team1 chan1 --subject Hi --file /dev/stdin` | Exit code 0; `sent` false; `would` has `team` "team1", `channel` "chan1", `subject` "Hi", `text` the text |
| 4 | Run `printf '…' \| target/debug/echoctl m365 teams send --chat 48:notes --file /dev/stdin` (the chat with yourself) | Exit code 0; `sent` false; `would` `{"chat":"48:notes","kind":"yourself","text":<the text>}` |
| 5 | Run `target/debug/echoctl m365 drive upload "$ECHOCTL_HOME/.echo/me.md" Documents/about-me.md` | Exit code 0; `sent` false; `would.to` "OneDrive/Documents/about-me.md", `would.file` the path given, `would.bytes` the file's size |
| 6 | Run step 1's command again with `--yes` | Exit code 1; "Error: not signed in to Microsoft 365 — `echoctl calendar login`"; nothing is sent |
| 7 | Run `printf '…' \| target/debug/echoctl m365 mail send --to not-an-address --subject x --file /dev/stdin` | Exit code 1; "Error: not a mail address: not-an-address" |

## Expected Final State
Nothing was sent or uploaded; every command without `--yes` described what it would send.

## Notes
- A send to a chat by its id (`--chat 19:…`) and a reply (`mail reply <id>`) read the chat or the mail first to show who would get it, so without a sign-in they stop with "not signed in" before any preview: covered by the person-run Microsoft 365 cases.
- With `--ai <model>` the message sent carries an AI notice that the preview's `text` doesn't show (it shows the Markdown as given).
