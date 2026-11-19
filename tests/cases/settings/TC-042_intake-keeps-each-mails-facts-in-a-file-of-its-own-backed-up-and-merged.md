---
id: TC-042
title: Intake keeps each mail's facts in a file of its own, backed up by the day, merged when read again, and gone only when its source leaves the thread
priority: high
type: functional
requirements: [F-KNOW-08]
module: settings
run: person
platform: any
tags: [settings, intake, data]
---

# TC-042: Intake keeps each mail's facts in a file of its own, backed up by the day, merged when read again, and gone only when its source leaves the thread

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- A person's run: a development build on a Mac signed in to a real Microsoft 365 mailbox (`run: person`: Graph returns mail only for a real mailbox), with its own data folder (`ECHOCTL_HOME`, `ECHOCTL_CONFIG_DIR` set to a scratch folder, never `~/echo`)
- A thread with a mail source (`domain:<a domain that mails you>`), a client, and that domain's sender trusted in Settings → Intake → Who may speak
- Settings → Intake → Use Intake on

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Press "Sync now" in Settings → Intake and wait until "Syncing…" is gone | "Last read" says today |
| 2 | List `$ECHOCTL_HOME/recall/intake/facts/mail/` | One `.json` per conversation read; each has `title`, `refs.threads` (the thread), `sections` with `head`, `from`, `at`, `link`, `sender` and `facts` — no message text |
| 3 | Press "Sync now" again after a new mail in one of those conversations arrived | That conversation's file has the new section and still every section it had before |
| 4 | List `$ECHOCTL_HOME/recall/intake/facts.bak/` | A folder named by today's date with the facts as they were before the last change |
| 5 | Take the mail source off the thread (its Profile), press "Sync now" | The thread's mail files are gone from `facts/` (still in today's backup); with more than a fifth of all files leaving at once, none goes and Echo's log says the removal was refused |

## Expected Final State
The facts read are kept in their files and backed up; they left only with their source.

## Teardown
- Remove the scratch data folder.

## Notes
- The rules behind each step are Rust tests on made-up data: `intake::facts::tests` (round trip, generations, merge, the move from the index) and `intake::sources::tests` (removals only with every thread read, the mass-removal refusal, Trust and Ignore, a bad id).
