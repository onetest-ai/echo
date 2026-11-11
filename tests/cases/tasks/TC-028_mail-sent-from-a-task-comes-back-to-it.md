---
id: TC-028
title: Mail sent from a task is followed by it — "no reply yet" on the item, Waiting after 3 days, and the reply comes back to the task
priority: high
type: functional
requirements: [F-M365-07, F-TASK-14]
module: tasks
run: person
platform: any
data: small
tags: [tasks, m365, mail, waiting, threads]
---

# TC-028: Mail sent from a task is followed by it — "no reply yet" on the item, Waiting after 3 days, and the reply comes back to the task

**Module:** Tasks | **Priority:** High | **Type:** Functional

## Preconditions
- Two mailboxes, both the person's own, made for this test, nothing real in them: **A**, a Microsoft 365 test account, the user (`<A>`); **B**, another account of the same organisation, a colleague (`<B>`)
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. Account A signed in to the run's own config: `tests/fixtures/serve.sh up <dir> <port>`, in the window Settings → Calendar & chat → connect Microsoft 365 (as `calendar/TC-002`)
- Why it is a person's case: it sends a real mail and reads the real reply (`m365::mail_send`, `graph::conversation`); the rules are `ledger::tests::mail_sent_from_an_item_is_followed_until_someone_answers` and `tasks::tests::a_mail_sent_from_a_task_waits_for_its_reply_after_three_days`
- A file `msg.md` with `Test mail, nothing real. Which date works for the cut-over?`

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout (`acme-rollout`) |
| Task | **Send Acme the SSO checklist** — its id from `target/debug/echoctl project ledger acme-rollout tree` (here `<T>`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl m365 mail send --to <B> --subject "Acme cut-over date" --file msg.md --task acme-rollout/<T>` | Nothing sent: `"sent": false`, and `"would"` holds `"task": "acme-rollout/<T>"` |
| 2 | Run `target/debug/echoctl m365 mail send --to <B> --subject "Acme cut-over date" --file msg.md --task acme-rollout/nope` | It fails: "no item nope in thread acme-rollout"; nothing sent |
| 3 | Run step 1's command with `--yes` | `"sent": true`, `"task": "acme-rollout/<T>"`, `"followed": true`; the mail is in B's Inbox |
| 4 | In the window open the task (`#project/acme-rollout/work`, the task's card) | A row "Mail" reading "Mail sent <today> to <B>, no reply yet" |
| 5 | From B, reply to the mail: `Test reply, nothing real. The 24th works.` Then run `target/debug/echoctl project ledger acme-rollout sync --mail` | `"written"` holds one `mail-…` id; `grep -l "answers" $ECHOCTL_HOME/projects/acme-rollout/ledger/evidence/mail-*.md` names its file, which says `answers: <T> “Send Acme the SSO checklist” (a reply to mail Me sent from it)` |
| 6 | Open the task again | The Mail row reads "Mail sent <today> to <B> · replied <today>" |

## Expected Final State
The task follows its mail's conversation; B's reply is evidence of the thread that names the task, and the task shows it was answered.

## Teardown
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)`
- Delete the test mails from A and B

## Notes
- Tasks → Waiting after 3 days without a reply is not walked here (it takes 3 days): `tasks::tests::a_mail_sent_from_a_task_waits_for_its_reply_after_three_days`.
- The reply's words reach the task through the thread's sync (the extractor reads the `answers:` line and clears or updates its wait); that run is the sync's own case.
- A reply from an outside sender waits in the Inbox as any mail (`privacy/TC-009`); trusted, it comes back to the task.
