---
id: TC-029
title: A mail source limited to subjects reads only the mail about the thread, and the rest of a conversation it started
priority: high
type: functional
requirements: [F-THR-21]
module: threads
run: person
platform: any
data: small
tags: [threads, m365, mail, sources]
---

# TC-029: A mail source limited to subjects reads only the mail about the thread, and the rest of a conversation it started

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Two mailboxes, both the person's own, made for this test, nothing real in them: **A**, a Microsoft 365 test account, the user (`<A>`); **B**, another account of the same organisation, a colleague (`<B>`)
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. Account A signed in to the run's own config: `tests/fixtures/serve.sh up <dir> <port>`, in the window Settings → Calendar & chat → connect Microsoft 365 (as `calendar/TC-002`); Recall on (Settings → Knowledge)
- Why it is a person's case: Graph returns mail only for a real mailbox (`graph::mail`)
- From B send A two mails, text `Test mail, nothing real.`: subject `Acme Platform status` and subject `Lunch on Friday`. When both are in A's Inbox, reply from B to the first with its subject changed to `Quick question`

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | In the window open `#project/acme-rollout/profile`, press ✉ Mail in the Sources row, type `<B>` in the address field and `Acme Platform` in the subject field, press Enter | A chip "✉ mail with <B> · subject: Acme Platform" |
| 2 | Run `target/debug/echoctl project ledger acme-rollout sync --mail` | JSON whose `"written"` holds two `mail-…` ids; `"mail"` lists `person:<B>` with `"found": 2` and no `"error"` |
| 3 | Run `grep -h '"title"' $ECHOCTL_HOME/projects/acme-rollout/ledger/evidence/mail-*.json` | "Acme Platform status" and "Quick question" (the reply in its conversation); no "Lunch on Friday" |

## Expected Final State
Acme rollout holds the two mails of the Acme Platform conversation; the lunch mail from the same colleague isn't in it.

## Teardown
- `target/debug/echoctl project source acme-rollout --remove mail:person:<B>`; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)`
- Delete the test mails from A and B

## Notes
- Nothing goes to anyone but the person's own two mailboxes.
