---
id: TC-020
title: compose puts a mail draft where drafts go — the default mail app, a chosen app, or (not connected) the mail app instead of Microsoft 365, saying why — never sends, and a run can't change where drafts go
priority: high
type: functional
requirements: [F-CLI-12]
module: cli
run: agent
platform: any
data: small
tags: [cli, mail, drafts, compose]
---

# TC-020: compose puts a mail draft where drafts go

**Module:** CLI | **Priority:** High | **Type:** Functional

## Preconditions
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. Microsoft 365 is not signed in (the test data's default)
- The data's environment catches drafts: a development build writes each `mailto:` link to `<dir>/mail-drafts.log` (`ECHOCTL_TEST_MAIL_DRAFTS`) instead of opening a mail app — nothing opens, nothing is sent
- A file `<dir>/slots.txt` with two lines: `Hi Kim,` and `which training slots work for you?`
- The chosen app's name, `<APP>`: on macOS `Spark`; on Windows the full path of a program that exists, here `target\debug\echoctl.exe` as its full path (only its name is checked: the draft goes to the log)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl config --drafts mail-app` | The config printed has `"drafts": "mail-app"` |
| 2 | `echoctl compose --to kim@acme.example --subject "Training slots?" --body-file <dir>/slots.txt`; then `tail -1 <dir>/mail-drafts.log` | `{"draft": true, "where": "mail-app", "to": ["kim@acme.example"], "subject": "Training slots?"}`; the log's last line is `mailto:kim@acme.example?subject=Training%20slots%3F&body=Hi%20Kim%2C%0Awhich%20training%20slots%20work%20for%20you%3F` (both lines of the body) |
| 3 | `echoctl config --drafts "app:<APP>"`; `echoctl compose --to kim@acme.example --subject Again --body-file <dir>/slots.txt`; `tail -1 <dir>/mail-drafts.log` | `"drafts": "app:<APP>"`; compose answers `"where": "app"` with the app's name; the log's last line starts `[<APP>] mailto:kim@acme.example?subject=Again` |
| 4 | `echoctl config --drafts m365`; `echoctl compose --to kim@acme.example --subject Third --body-file <dir>/slots.txt`; `tail -1 <dir>/mail-drafts.log` | `"where": "mail-app"` and a `"note"` starting `not in Microsoft 365 Drafts (` and ending `): opened in the mail app instead`; the log's last line starts `mailto:kim@acme.example?subject=Third` |
| 5 | `echoctl config --drafts pigeon`; then `ECHOCTL_HEADLESS=1 echoctl config --drafts mail-app`; then `echoctl config` | The first fails: `where drafts go: m365, mail-app or app:<app>`; the second fails: `refused inside a run Echo started: where drafts go is the user's to choose`; the config still has `"drafts": "m365"` |
| 6 | `echoctl config --drafts default`; `echoctl config` | No `"drafts"` in the config: Echo's default (here, not connected: the default mail app) |

## Expected Final State
Three `mailto:` lines in `<dir>/mail-drafts.log`; nothing opened or sent; where drafts go is Echo's default again.

## Teardown
- Remove `$(dirname <dir>)` when the run is over

## Notes
- A draft in Microsoft 365 Drafts needs a sign-in: `today/TC-039` (a person's case). The rules are `drafts::tests` and `m365::tests::a_run_echo_started_may_leave_a_draft_but_never_sends_replies_or_posts`.
