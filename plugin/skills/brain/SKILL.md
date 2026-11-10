---
name: brain
description: "Use when the user asks what was said, decided or promised across calls and documents (\"what did we agree with Acme about…\", \"when did X first come up\", \"which calls mention…\"), for a figure from a client's reports, before a brief or meeting prep, or to fix or rebuild the Brain's topics and terms. Answers from Echo's Brain — every call, the threads' mail, channels and folders, by client and thread — with links back to the moment."
argument-hint: "[question or what to change]"
---

# echo: the Brain

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl brain` (below: `brain`). If the session has the `echo-brain` MCP tools
(`echo-brain:search_knowledge`, `echo-brain:read_document`, `echo-brain:get_evidence`, `echo-brain:list_metrics`,
`echo-brain:get_metric`), prefer them: they answer the same, scoped to this session's profile. A command that answers
that Recall is off → say so (Settings → Knowledge → Use Recall) and stop.

## Answering a question

```
- [ ] 1. Scope: which client or thread is it about? `brain scopes` lists their ids (or `echoctl project list`)
- [ ] 2. Search: brain search "<the question's words>" --client <id> | --thread <id> [--limit 12]
- [ ] 3. Search again in other words — the other language (calls mix English, Ukrainian, Russian), a person's name,
         a product or project term
- [ ] 4. Follow the best hits: brain related <section id> (the same thing said elsewhere)
- [ ] 5. Answer: each point with who said it, when and its link (echo://call/…?t=… opens the moment in Echo);
         "not in the Brain" when nothing supports it — never fill gaps from memory
```

- Search fuses keywords and meaning (when an embedding server is set): phrase the query as the content would say it,
  not as a question.
- `--within call/2026-10` narrows to sources whose id contains it (one month of calls, one mailbox thread).
- Hits marked `closed` come from closed threads (the archive); `history` ones are older than the thread's history
  date. Say so when you use them.
- Mail and channel content is stored as facts with short quotes, not the messages themselves: for the full text, the
  `echo:microsoft-365` skill opens the mail or chat.

## Numbers

A client's governed metrics come from its spreadsheets; a number is never estimated.

```sh
brain numbers list                                    # the metrics, per client
brain numbers get "<metric>" --client <id> [--entity …] [--month 2026-09] [--grain …]
```

Quote the value with its file and month; `restated` or `conflicting` means the reports disagree — say which says what.
Setting up a client's numbers (`numbers profile | propose | accept | build`) runs Claude on its workbooks: only when
the user asks.

## Topics and terms

```sh
brain taxonomy show                   # every topic and term with sections and threads
brain taxonomy drift                  # which layers no longer fit their calls (no model)
brain taxonomy moves [--apply KEY | --dismiss KEY]    # topics in the wrong layer
brain taxonomy edit '<json>'          # the user's change: a new version, nothing rebuilt
```

Edits (`layer`: `shared` = the practice, or a client's id; `term: true` for a term, else a topic):

```json
{"op":"rename","label":"Pricing","to":"Commercials"}
{"op":"merge","label":"Costs","into":"Commercials"}
{"op":"move","label":"Pilot plan","parent":"Delivery"}
{"op":"set","label":"Commercials","description":"How the work is priced and paid"}
{"op":"add","term":true,"name":"ServiceNow","kind":"product","synonyms":["SNOW"],"variants":["Service Now"]}
{"op":"set","term":true,"name":"ServiceNow","variants":["Service Now","servicenow"],"group":"product"}
{"op":"scope","label":"Pilot plan","to":"shared"}
{"op":"remove","label":"Small talk"}
{"op":"accept","layer":"acme"}
```

A term's **synonyms** are other names (search and Claude), its **variants** spellings and mishearings (they hint
transcription). Rebuilding or splitting (`taxonomy build --layer …`, `taxonomy split <layer> <topic>`, `classify`)
runs Claude over many sections: say what it will do and how long, and run it only when the user asks.

## Who may speak, and the rest

```sh
brain senders [--client <id>]                         # mail and channel senders: trusted, ignored, waiting
brain senders --client <id> --trust jane@acme.example | --ignore … | --forget …
brain insights --client <id> | --thread <id> [--run]  # what moved, slips, recurs, contradicts (--run: refresh now)
brain files                                           # the threads' folders: what was read, and why not
brain sync                                            # read what's new now (it also runs every 5 minutes)
brain set --enabled true|false                        # the Brain at all
```

Trusting a sender makes Echo read their mail and posts for that client from now on: ask first.
