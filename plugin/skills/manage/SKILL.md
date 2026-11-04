---
name: manage
description: "Manages the user's Echo workspace through its command line: clients and organisations, threads (Echo's name for a client engagement or project, with the rules that put calls in them) and their stage, a thread's work (stories, tasks, decisions, risks, questions), people, open tasks across threads, calls with their minutes and action items, the calendar, Brain search, and mail sent through Echo's own Microsoft 365 sign-in. Use when the user asks to create, change, close, find or send something about their clients, threads, calls, minutes, action items or tasks — \"add a client\", \"make a thread for…\", \"close the Acme thread\", \"put this call in…\", \"mark that task done\", \"who owes me what\", \"email X about…\", \"what did we decide about…\" — whether or not they say \"Echo\". Prefer it to an issue tracker's tools for these words unless the user names that tracker."
argument-hint: "[what to do]"
---

# echo: manage

Echo keeps the user's calls, threads, clients, work and people; this skill changes and reads them through its CLI.
CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Answers are JSON. Any command takes `--help`;
when a flag here isn't enough, read its help rather than guessing. The Echo window shows every change at once.

Echo the app must be running for a few things (recording, minutes, mail): `echoctl status` — `watcher` null means
it isn't; `echoctl app start`. If a command fails for a reason that isn't the input, run the `doctor` skill.

## Rules

- **Ask before what leaves the computer or can't be undone**: sending mail, deleting a thread or a client,
  rejecting a suggestion for good. Say exactly what will happen (recipients, text, what is removed), then do it
  only after the user's yes.
- **Name things by their names**, never by ids: say "the task *Send the SOW draft*", not `T043`. Ids are for the
  commands only.
- **Look before you change**: list first (`client list`, `project list`, `project show <id>`, the ledger's `tree`)
  to find the id and to avoid making a second one of something that exists.
- Words "thread" and "project" are the same thing: the window says Threads, the CLI says `project`.
- Never edit Echo's files by hand (`projects/*.json`, `meta.json`, the ledger's `events.jsonl`): only the CLI keeps
  them consistent.

## A new client with a thread

The most common request. Copy the checklist and tick it off:

```
- [ ] 1. Look: `client list`, `project list` — is there one already (another name, a domain in common)?
- [ ] 2. Client: `client add "<name>" --domains <their mail domains>`
- [ ] 3. Thread: `project create "<name>" --client <client id>`
- [ ] 4. Rule: `project rule <thread id> --series "<meeting>"` or `--people` (see Threads)
- [ ] 5. Check: `project show <thread id>` — the calls it took; tell the user which, and fix the rule if wrong
```

Only report done after step 5 shows what the user meant.

## Clients and organisations

An **organisation** is any company people come from; a **client** is an organisation threads are done for
(`--account`). Domains decide whose people are whose.

```sh
echoctl client list                                   # clients with their domains
echoctl client add "Acme Corp" --domains acme.example,acme-group.example
echoctl client set <id> --name "Acme" --domains acme.example   # --domains replaces the list
echoctl client remove <id>                            # only a client without threads
echoctl org list                                      # every organisation, clients included
echoctl org add "Globex" --domains globex.example [--account] [--thread <thread id>]   # --thread: its counterpart there
echoctl org set <id> --parent <org id> --aliases "GX,Globex Ltd" --thread <thread id> | --unlink <thread id>
```

The user's own company is a client with `--internal` (`client add … --internal`): its people count as colleagues.

## Threads

```sh
echoctl project list                                  # every thread: client, stage, calls
echoctl project show <id>                             # its calls and why each belongs, rules, sources, people
echoctl project create "Acme rollout" --client <client id> [--stage kickoff]
echoctl project set <id> --stage done --reason "Signed off 4 Nov"   # closes it; reopen: --stage active
echoctl project set <id> --name … --client <id> --since 2026-09-01 --notes "What Claude should know"
echoctl project set <id> --brief brief.md             # the Brief tab, from a file ("-": stdin)
echoctl project delete <id>                           # to the trash; its calls stay — ask first
```

Stages: lead, proposal, negotiation (pursuit) · kickoff, active, on_hold, wrap_up (delivery) · done, lost,
cancelled (closed). A closed thread keeps what it has and takes no new calls.

**Which calls belong** — rules, computed every time (a call is never "moved"):

```sh
echoctl project rule <id> --series "Acme weekly sync"     # every occurrence of a recurring meeting
echoctl project rule <id> --people                         # half the call's outside people from the client's domains
echoctl project rule <id> --people acme.example other.example
echoctl project rule <id> --subject "rollout"              # subject contains these words
echoctl project rule <id> --remove 2                       # by number, as `show` lists them
echoctl project include <id> <call>                        # pin one call
echoctl project exclude <id> <call>                        # take one out (rules won't put it back)
echoctl project reset <id> <call>                          # back to the rules
echoctl project of <call>                                  # the threads a call is in, and may be in
echoctl project found                                      # threads Echo would make from calls in none
echoctl project found --accept <key> --name "…" --client <id|new|"">   |   --merge <key> --into <id>   |   --reject <key>
```

Pick the narrowest rule that says what the user means: a recurring meeting → `--series`; "everything with Acme" →
`--people`. After adding a rule, `project show` tells which calls it took — check it with the user.

**Sources** the thread reads besides calls (mail and channels into its work and the Brain):

```sh
echoctl project source <id> --mail acme.example            # a domain or an address
echoctl project source <id> --chat 19:…@thread.v2 --name "Acme weekly chat"
echoctl project source <id> --channel <team id>|<channel id> --name "Acme / General"
echoctl project source <id> --folder ~/Documents/Acme --exclude "*.tmp"
echoctl project source <id> --remove mail:acme.example     # kind:id, as `show` lists them
```

Glossary and people of a thread: `project term <id> "ServiceNow" "RACI" [--kind system]` (hints transcription);
`project person <id> "Jane Doe" --email jane@acme.example --role "Sponsor"`.

## A thread's work

Stories (with acceptance criteria) › tasks, plus decisions, risks and questions, read from its calls by the sync.

```sh
echoctl project ledger <id> tree                     # the whole tree (JSON): find nodes here
echoctl project ledger <id> status                   # the last sync
echoctl project ledger <id> run                      # sync now: its new calls, then Claude reads them
echoctl project tasks                                # open tasks across threads: I owe / owed to me / waiting / nobody named
echoctl project people <id> [--refresh]              # its people: calls, talk share, tasks, roles
```

Changes are the user's edits — they pin the field, and the sync never overwrites them:

```sh
echoctl project ledger <id> edit '{"op":"edit","node":"<node id>","set":{"state":"done"}}'
echoctl project ledger <id> edit '{"op":"edit","node":"<id>","set":{"internal_owner":"Jane Doe","due":"2026-11-14"}}'
echoctl project ledger <id> edit '{"op":"create","kind":"task","parent":"<story id>","title":"Send the SOW draft","internal_owner":"Me","due":"2026-11-10"}'
echoctl project ledger <id> edit '{"op":"create","kind":"decision","title":"Pilot in one region first","description":"Why, and the options weighed"}'
echoctl project ledger <id> edit '{"op":"merge","node":"<id>","into":"<id>"}'
echoctl project ledger <id> edit '{"op":"undo","target":"<mid of the edit>"}'
```

- `kind`: epic, story, task, risk, decision, question. A task's `parent` is a story; a story's, an epic.
- `set` fields: title, state (open, in_progress, blocked, done, cancelled), internal_owner, client_owner, due
  (YYYY-MM-DD), parent, type (action, session, dependency), client_owed, waiting_for, links (`[{title, url}]`,
  https), description, delegated_by, decided_by, likelihood / impact (low, med, high).
- "Done", "Me", owners: use the names the tree uses. The user is "Me".

## Calls

```sh
echoctl status                                       # recording? armed? the app running?
echoctl prep [call]                                  # last time: open actions and highlights of a recurring call
echoctl context <call>                               # what a call holds (sizes, people, languages)
echoctl actions <call>                               # its action items, each with an id
echoctl action <call> <id> done|todo
echoctl title <call> "Acme kickoff"
echoctl notes <call>                                 # write (or rewrite) its minutes now — Echo runs it
echoctl note "text"                                  # a note into the call being recorded
echoctl calendar events --days 3                     # the user's meetings
echoctl calendar import "<subject words>" [--date 2026-11-03]   # a meeting nobody recorded, from its Teams transcript
echoctl attention                                    # what waits for the user's decision (the Inbox)
```

`<call>` is a call's folder name (from the calls list, `project show`, or `latest`). Recording itself is the
`record` skill.

## Mail

Always through Echo first: it sends from the user's own Microsoft 365 account with Echo's sign-in.

1. Write the message as a Markdown file. Show the user the recipients, subject and text; wait for their yes.
2. `echoctl deliver status` — mail ready?
3. `echoctl deliver send mail --to a@x.example,b@y.example --title "Subject" --file message.md`
   (without `--to`: to the user themselves). Add `--ai <model>` when you wrote the text: the mail says so.

Only if Echo can't send (status says mail isn't ready, or the send failed): a Microsoft 365 / Outlook connector of
your own, if one works here; else `echoctl compose --to … --subject … --body-file message.txt`, which opens a draft
in the user's mail app for them to send.

## The Brain (when it is on)

`echoctl brain search "what we decided about the pilot" [--client <id>] [--thread <id>]` — words and meaning,
across calls, mail, channels and folders; each hit links to its source. Scope it to the client or thread asked
about. If the session has the `echo-brain` MCP tools, prefer them. "Not in the Brain" is a valid answer.
