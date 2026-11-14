---
id: TC-039
title: Yolo on a task that writes a mail leaves it in Microsoft 365 Drafts — the right To, Subject and body, nothing sent — and its outcome links to the draft
priority: high
type: functional
requirements: [F-M365-08, F-TASK-12]
module: today
run: person
platform: any
data: small
tags: [today, yolo, mail, drafts, m365]
---

# TC-039: Yolo leaves a mail draft in Microsoft 365 Drafts

**Module:** Today | **Priority:** High | **Type:** Functional

## Preconditions
- A Microsoft 365 test account, the person's own, made for this test, nothing real in it (`<A>`); a second test address `<B>` to write to
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; `tests/fixtures/serve.sh up <dir> <port>`; account A signed in to the run's own config (Settings → Calendar & chat → connect Microsoft 365, as `calendar/TC-002`); Settings → General → Where drafts go left at "Echo's default" (its line: "Now: Microsoft 365 Drafts …")
- Why it is a person's case: a real draft in a real mailbox (`m365::draft_text`), and Yolo runs Claude Code as the folder session, with "Claude in the window: no permission prompts" switched on for it, on a model of the person's choosing (this test data only)
- A task of Acme rollout whose doing is a mail: `target/debug/echoctl project ledger acme-rollout edit '{"op":"create","kind":"task","parent":"S001","title":"Ask <B> which training slots work","internal_owner":"Me"}'` (its id `<T>`)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | On Today, in My work · this week, open the task's Claude dialog and press Yolo | "Yolo: Claude is on it…", then the outcome above My work |
| 2 | Read the outcome | Claude's sentence, and a line "A draft in Microsoft 365 Drafts — “<subject>” to <B>" with a link "Open the draft ↗" |
| 3 | Follow the link (Outlook on the web), then look in A's Drafts and Sent Items | The draft opens: To `<B>`, the subject and the body Claude wrote; it is in Drafts; nothing in Sent Items, nothing in B's inbox |
| 4 | `cat <dir>/config/yolo/ask-*/log.txt` | If the run tried `m365 mail send` or `reply`, a "✗ … refused inside a run Echo started" after it; `compose` without "✗" |

## Expected Final State
One draft in A's Drafts; nothing sent; the outcome says where the draft is and links to it.

## Teardown
- Delete the draft in A's Drafts; `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)`

## Notes
- With "The default mail app" chosen instead, the same run's draft goes to the mail app (on test data: `<dir>/mail-drafts.log`), and the outcome says "A draft opened in your mail app": `tasks/TC-026`.
