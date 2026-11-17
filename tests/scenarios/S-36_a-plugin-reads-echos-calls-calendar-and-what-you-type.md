---
id: S-36
title: "A plugin on Echo's data: the calls and a call's facts, the calendar and you, Microsoft 365 through Echo's sign-in, a page you type, a chip on a call's row"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-100_a-plugin-lists-the-calls-as-its-sheet-says.md
  - tests/cases/plugins/TC-101_a-plugin-reads-a-calls-facts-its-chat-and-who-was-invited.md
  - tests/cases/plugins/TC-102_a-plugin-reads-the-meetings-echo-read-and-who-the-user-is.md
  - tests/cases/plugins/TC-103_a-plugins-microsoft-365-reads-need-echos-sign-in-and-its-sheet.md
  - tests/cases/plugins/TC-105_a-plugin-fetches-the-address-the-user-typed-once.md
  - tests/cases/plugins/TC-106_a-plugins-chip-on-a-calls-row-in-the-calls-list.md
  - tests/cases/plugins/TC-104_a-plugin-reads-a-mail-thread-with-its-attachments-and-a-chats-days.md
  - tests/cases/plugins/TC-160_a-plugins-row-in-a-clients-profile-and-its-values-per-client.md
---

# S-36: A plugin on Echo's data

**Story:** Sam installs a plugin that files calls into a notes vault. Its sheet says what it reads: the list of calls, a
call's minutes with who was there and the meeting chat, today's meetings, Sam's own name, Sam's mail and chats through
Echo's sign-in — and a page Sam types the address of. It reads exactly that and no more; its chip marks the calls in
the list it cares about. Another plugin of the same kind keeps a vault per client, on each client's Profile.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (by 22:50, else `--now`).

**Length:** about 10 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build | the lead (`serve.sh up`) | 1–6 |
| The marketplace `test-plugins` and Data demo installed through its sheet | stage 1 (step 1) | 1–6 |
| The made-up web site on port 48139 | stage 5 (its preconditions) | 5 (stopped by its teardown) |

## Prelude

1. Load `{{base_url}}/`.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-100_a-plugin-lists-the-calls-as-its-sheet-says.md` | — | Skip its teardown: keep Data demo for stages 2–6 |
| 2 | `plugins/TC-101_a-plugin-reads-a-calls-facts-its-chat-and-who-was-invited.md` | 1 | Data demo is installed: skip its install precondition |
| 3 | `plugins/TC-102_a-plugin-reads-the-meetings-echo-read-and-who-the-user-is.md` | 1 | Data demo is installed: skip its install precondition |
| 4 | `plugins/TC-103_a-plugins-microsoft-365-reads-need-echos-sign-in-and-its-sheet.md` | 1 | Data demo is installed: skip its install precondition |
| 5 | `plugins/TC-105_a-plugin-fetches-the-address-the-user-typed-once.md` | 1 | Data demo is installed: skip its install precondition; run its site lines |
| 6 | `plugins/TC-106_a-plugins-chip-on-a-calls-row-in-the-calls-list.md` | 1 | Data demo is installed: skip its install precondition. Its teardown, as the scenario's last: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins` |
| 7 | `plugins/TC-104_a-plugin-reads-a-mail-thread-with-its-attachments-and-a-chats-days.md` | — | **person** (a real Microsoft 365 account) |
| 8 | `plugins/TC-160_a-plugins-row-in-a-clients-profile-and-its-values-per-client.md` | — | Its own plugin (Client demo) from the same marketplace: add `test-plugins` again only if stage 6's teardown removed it; its teardown removes it |

## Notes
- One install serves every stage: the reads change nothing, and stage 6 leaves the plugin on as it found it.
