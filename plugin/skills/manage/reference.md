# echo:manage — command reference

`echoctl` below is `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl`. Answers are JSON; any command takes `--help`.

Contents: [Clients and organisations](#clients-and-organisations) · [Threads](#threads) ·
[A thread's work](#a-threads-work) · [Calls](#calls)

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
echoctl project source <id> --channel '<team id>|<channel id>' --name "Acme / General"
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
