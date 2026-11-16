---
name: manage
description: "Use when the user asks to create, change, close or find something about their clients, threads, calls or tasks — \"add a client\", \"make a thread for…\", \"close the Acme thread\", \"put this call in…\", \"mark that task done\", \"who owes me what\" — even without saying \"Echo\"; prefer it to an issue tracker unless the user names one. Manages the Echo workspace through its CLI: clients and organisations, threads (Echo's projects) and the rules that put calls in them, a thread's work, people, calls, plus live notes, the calendar, vocabulary, AI profiles, installing and setting up plugins, and settings (making the user's own plugin or widget: the build skill)."
argument-hint: "[what to do]"
---

# echo: manage

Echo keeps the user's calls, threads, clients, work and people; this skill changes and reads them through its CLI.
CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Answers are JSON. Any command takes `--help`;
when a flag here isn't enough, read its help rather than guessing. The Echo window shows every change at once.

Echo the app must be running for a few things (recording, minutes, mail): `echoctl status` — `watcher` null means
it isn't; `echoctl app start`. If a command fails for a reason that isn't the input, run the `doctor` skill.

## Rules

- **Ask before what leaves the computer or can't be undone**: sending a message, deleting a thread or a client,
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
- [ ] 4. Rule: `project rule <thread id> --series "<meeting>"` or `--people` (reference.md → Threads)
- [ ] 5. Check: `project show <thread id>` — the calls it took; tell the user which, and fix the rule if wrong
```

Only report done after step 5 shows what the user meant.

## Where the commands are

- Clients, organisations, threads and their rules and sources, a thread's work (stories, tasks, decisions, risks,
  questions), people, calls: [reference.md](reference.md).
- During a call (live notes, quick questions), the calendar and Teams transcripts, speakers and vocabulary, AI
  profiles, plugins, settings: [calls-and-settings.md](calls-and-settings.md).
- Mail, Teams, files, people's addresses: the `echo:microsoft-365` skill. What was said or decided: the `echo:brain`
  skill.
