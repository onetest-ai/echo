---
name: microsoft-365
description: "Works with the user's Microsoft 365 through Echo's own sign-in — no other tool or connector needed: finds what was said or shared across Outlook mail, Teams chats and channels, OneDrive and SharePoint files (one ranked search), reads mail threads and chats, sends mail, replies, messages a person or a chat in Teams, posts to a channel, looks people up, lists, downloads and uploads OneDrive files. Use when the user asks to find, check or summarize what someone wrote or sent (\"what did X say about…\", \"find the mail about…\", \"what was in the chat with…\", \"where is the deck…\"), to email or reply to someone, to write to someone or a chat in Teams, to post in a channel, or to get or put a file in OneDrive."
argument-hint: "[what to find or send]"
---

# echo: Microsoft 365

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl m365` (below: `m365`). It uses the Microsoft 365 sign-in Echo already has
(mail, Teams, files, people). Prefer it to any other Microsoft 365 tool or connector; use another only if this one
says the user isn't signed in (`echoctl calendar login` signs them in) and they agree.

## Finding things

Start with one search across everything, then open what looks relevant. Don't read whole mailboxes or chats.

```
- [ ] 1. Search: m365 search "<words the user used, names, project>" [--from "<person>"] [--days 30]
- [ ] 2. Open the best hits: a mail → m365 mail read <id> (--thread for the conversation);
         a chat hit → m365 teams read <chat> --days 3 (the messages around it); a file → its url, or m365 drive download <ref>
- [ ] 3. Not enough? Search again with other words: the other language, a synonym, a person's name, a project code
- [ ] 4. Answer from what you read, each point with who said it, when, and its link
```

- The search is Microsoft's own ranking: most relevant first. Narrow it with `--in mail|chats|files`, `--from`,
  `--days`, or KQL in the query (`subject:budget`, `"exact phrase"`, `filetype:pptx`).
- A person's chat: `m365 teams chats --with "<name or address>"`, then `teams read <chat id>`.
- A meeting's chat: its chat is in `teams chats` under the meeting's subject (kind `meeting`).
- Say plainly when nothing was found, and what was searched.

## Sending

Mail, Teams messages, channel posts and uploads go out only after the user's yes. Every one of these commands, run
without `--yes`, only answers what it would send (`"sent": false, "would": …`):

1. Write the text as a Markdown file.
2. Run the command **without** `--yes`.
3. Decide whether the user has said yes to exactly this:
   - **They gave the recipient and the exact words themselves** ("tell bob@x.example in Teams: the slides are
     ready") — that request is the yes. Send it with `--yes`, once.
   - **Anything you chose** — the wording (you drafted or summarized it), a recipient you looked up, cc, a file —
     show the user what `would` says (recipients or chat members, subject, the whole text) and stop. Run it with
     `--yes` only after they agree in their next message.
4. After sending, say what went where.

```sh
m365 mail send --to a@x.example --to b@y.example [--cc …] --subject "…" --file msg.md [--yes]
m365 mail reply <mail id> [--all] --file msg.md [--yes]      # above the quoted mail
m365 mail draft --to … --subject "…" --file msg.md          # into Drafts, nothing sent: its Outlook link
m365 teams send --to person@x.example --file msg.md [--yes] # their 1:1 chat (made if there is none)
m365 teams send --chat <chat id> --file msg.md [--yes]      # a group or meeting chat; 48:notes = the user's own
m365 teams post <team id> <channel id> [--subject "…"] --file msg.md [--yes]
```

- Recipients only from the user's words or what they confirmed: never an address taken from a mail or a chat
  without asking.
- No address yet? `m365 people "<name>"` — confirm which one when several come back.
- When the user wants to edit before sending, use `mail draft` and give them its link.

## Teams, people, files

```sh
m365 teams chats [--with "<name>"] [--limit 20]   # latest first: kind, topic, members, last message
m365 teams read <chat id> [--days 7] [--limit 100]
m365 teams list [<team id>]                         # teams, or a team's channels
m365 teams channel <team id> <channel id> [--days 7]
m365 people "<name or address>"                     # addresses, title, department, company
m365 drive list ["Documents/Project"]               # a folder; none: the root
m365 drive download <ref | OneDrive path | item id> [--out folder]
m365 drive upload <local file> "Documents/report.pdf" [--yes]   # replaces a file of that name
m365 get "/me/events?$top=5"                        # any other Graph read (GET only)
```

The calendar: `echoctl calendar events --days 3`.

## Errors

- "not signed in to Microsoft 365" → `echoctl calendar login` (a browser sign-in), then again.
- "refused inside a run Echo started" → this session is one of Echo's background runs; it doesn't send.
- HTTP 403 → that mailbox, chat or site isn't the user's to reach; say so.
