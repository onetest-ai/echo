---
id: TC-009
title: A thread reads mail from a colleague at once, and mail from an outside sender waits in the Inbox until the user trusts them
priority: high
type: functional
requirements: [F-M365-06]
module: privacy
run: person
platform: any
data: small
tags: [privacy, m365, mail, senders, threads]
---

# TC-009: A thread reads mail from a colleague at once, and mail from an outside sender waits in the Inbox until the user trusts them

**Module:** Privacy | **Priority:** High | **Type:** Functional

## Preconditions
- Three mailboxes, all the person's own, made for this test, nothing real in them:
  - **A**: a Microsoft 365 test account, the user; its address is `<A>`
  - **B**: another account of the same organisation (a colleague: its address ends in one of the organisation's domains); `<B>`
  - **C**: a personal address outside the organisation at a provider that passes DMARC (an outlook.com or gmail.com address); `<C>`
- Small data built right before the run: `tests/fixtures/make-data.sh <dir>`; every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`. Account A signed in to the run's own config: `tests/fixtures/serve.sh up <dir> <port>`, in the window Settings → Calendar & chat → connect Microsoft 365 (as `calendar/TC-002`); `<dir>/config/graph.json` exists
- The thread **Acme rollout** (`acme-rollout`, client Acme) on the data
- Why it is a person's case: Graph returns the mail and Exchange's authentication verdict only for a real mailbox (`graph::mail`); the verdicts alone are `brain::senders::tests`
- From B, send A one mail with the subject `Acme rollout check from colleague` and the text `Test mail, nothing real.`; from C, send A one with the subject `Acme rollout check from outside` and the same text. Wait until both are in A's Inbox (Outlook, any client)

## Test Data

| Field | Value |
|---|---|
| Colleague's mail | subject `Acme rollout check from colleague`, from `<B>` to `<A>` |
| Outside mail | subject `Acme rollout check from outside`, from `<C>` to `<A>` |
| Evidence folder | `$ECHOCTL_HOME/projects/acme-rollout/ledger/evidence/` (a mail is the file `mail-<8 hex>.json`) |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Run `target/debug/echoctl project source acme-rollout --mail <B>` and `target/debug/echoctl project source acme-rollout --mail <C>` | Each prints the thread's sources; the list now has `{"kind": "mail", "id": "person:<B>"…}` and `{"kind": "mail", "id": "person:<C>"…}` (addresses lower case) |
| 2 | Run `target/debug/echoctl project ledger acme-rollout sync --mail` | JSON whose `"written"` holds one id `mail-…`; `"mail"` lists both sources with no `"error"`: `person:<B>` with `"found": 1`, `person:<C>` with `"found": 0` |
| 3 | Run `grep -l "Acme rollout check" $ECHOCTL_HOME/projects/acme-rollout/ledger/evidence/mail-*.json` and `grep -h '"title"' ` on the file it names | One file, whose `"title"` is "Acme rollout check from colleague" and whose `"auth"` is "internal"; no file with "from outside" (`grep -l "from outside" …` prints nothing) |
| 4 | Run `target/debug/echoctl attention` | In `"open"` an item of kind `sender`, titled "<C's display name, or <C>> isn't read yet", its why starting "<C> · 1 message" and ending "· “Acme rollout check from outside”"; its actions are "Trust" and "Ignore" (no item for B) |
| 5 | In the window open the right panel's Inbox (`[data-panel-flip="needs"]`) | The same item: "isn't read yet" with the buttons Trust and Ignore |
| 6 | Click "Trust" on that item, then run `target/debug/echoctl project ledger acme-rollout sync --mail` | The item is gone from the Inbox; `"written"` holds one new `mail-…` id; `grep -l "from outside" $ECHOCTL_HOME/projects/acme-rollout/ledger/evidence/mail-*.json` names a file whose `"title"` is "Acme rollout check from outside" and whose `"auth"` is "pass" |

## Expected Final State
The thread holds the colleague's mail (read at once) and, only after "Trust" (step 6), the outside sender's; before that the outside mail was in no evidence file and sat as one waiting item in the Inbox.

## Teardown
- `target/debug/echoctl project source acme-rollout --remove mail:person:<B>` and `… --remove mail:person:<C>`
- `tests/fixtures/serve.sh down <dir>`; remove `$(dirname <dir>)` (the sign-in lived in `<dir>/config/graph.json` only)
- Delete the three test mails from the mailboxes A, B, C

## Notes
- Nothing goes to anyone but the person's own three mailboxes; no real person or client's mail is read (the account A holds only test mail, and a rule names only `<B>` and `<C>`).
- "Ignore" (the sender's mail then never read) is not walked here: it is the other button of step 5's item.
- Mail from `<C>` that did not pass authentication would not be read even after "Trust": it waits as "<name>'s mail isn't authenticated" with "Read anyway" / "Keep out"; not tested here (no way to send a forged mail from a real provider).
- Meeting chats and channel posts use the same list (`senders.teams`); their case is a thread's chat source with a message from a colleague and from an outside guest.
