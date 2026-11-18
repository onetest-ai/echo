---
name: brain
description: "Use when the user asks what was said, decided or promised across calls, mail, channels and documents (\"what did we agree with Acme about…\", \"when did X first come up\", \"which calls mention…\"), before a brief or meeting prep, or to fix the Brain's topics and terms. Answers from Echo's search — every call, the threads' mail, channels and folders, by client and thread — with links back to the moment."
argument-hint: "[question or what to change]"
---

# echo: what was said

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl brain` (below: `brain`). If the session has Echo's MCP tools
(`mcp__echo__search_sources`, `mcp__echo__read_source`, `mcp__echo__read_link`), prefer them: they answer the same,
scoped to this session's profile. A command that answers that Intake is off → say so (Settings → Intake → Use Intake)
and stop.

This skill answers only **what was said**: words, who said them, when, with a link to the moment. Who someone is, why
or how things relate, and long-term facts are the Knowledge plugin's (its vault and its skills): go there for those,
never guess them from search hits.

## Answering a question

```
- [ ] 1. Scope: which client or thread is it about? Its id: list_scopes (CLI: `brain scopes`, or
         `echoctl project list`)
- [ ] 2. Search: search_sources {"query": "<the question's words>", "client" | "thread": "<id>"}
         (CLI: brain search "<words>" --client <id> | --thread <id> [--limit 12])
- [ ] 3. Search again in other words — the other language (calls mix English, Ukrainian, Russian), a person's name,
         a product or project term
- [ ] 4. Read around the best hits: read_link <its echo:// link> (the turns around the moment), read_source <source>
         (the whole call, mail conversation or document)
- [ ] 5. Answer: each point with who said it, when and its link (echo://call/…?t=… opens the moment in Echo);
         "not found in Echo" when nothing supports it — never fill gaps from memory
```

- Search finds the query's words (common words like "what" or "що" are skipped) and, when an embedding server is set,
  orders those hits by meaning: phrase the query with the words the content would use, not as a question.
- `--within call/2026-10` (MCP: `source_contains`) narrows to sources whose id contains it (one month of calls, one
  mailbox thread).
- Filters (MCP: the same names; several values comma-separated, or a list):
  - `--kind call,mail,channel,document`: kinds of source.
  - `--section decisions,action_items`: kinds of section. The kinds are summary, decisions, action_items,
    open_questions, minutes (any other minutes section), transcript, chat, mail, channel, document and card.
  - `--since 2026-11-01 --until 2026-11-12`: days. Without them, a date in the query becomes the filter, and the hit
    list's `filters.dates_from_query` says so. Examples: "12 November", "2026-11-12", "last week", "12 листопада",
    "на прошлой неделе".
  - `--person Dana`: what that person said or wrote. Minutes and documents have no speaker, so leave it off for them.
  - A query that is only a day ("what happened yesterday") lists that day's sources, newest first, and its `note`
    says so. For English months, write them capitalised ("3 May"), or as `2026-05-03`.
- The index is working memory, not an archive:
  - Transcripts and chats are kept 45 days.
  - Minutes, mail facts and channel facts are kept 90 days.
  - A closed thread's sections leave it.
  - Every call keeps a **card**: its title, date, threads, people and the summary's first line.
  - A closed thread's document keeps a card: its title, path and headings.
  - When a hit is a card, or the note says "older than the index keeps", read the call itself through its link
    (read_link reads the call's folder, not the index), or look in Knowledge.
- Hits marked `closed` come from closed threads (the archive); `history` ones are older than the thread's history
  date. Say so when you use them.
- Mail and channel content is stored as facts with short quotes, not the messages themselves: for the full text, the
  `echo:microsoft-365` skill opens the mail or chat.
- Scanned PDFs and slides that are only pictures are indexed by their text alone: for what is in the picture, the
  Knowledge plugin reads the file.

## Topics and terms

```sh
brain taxonomy show                   # every topic and term with sections and threads
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
transcription).

## Who may speak, and the rest

```sh
brain senders [--client <id>]                         # mail and channel senders: trusted, ignored, waiting
brain senders --client <id> --trust jane@acme.example | --ignore … | --forget …
brain files                                           # the threads' folders: what was read, and why not
brain sync                                            # read what's new now (it also runs every 5 minutes)
brain set --enabled true|false                        # Intake at all
```

Trusting a sender makes Echo read their mail and posts for that client from now on: ask first.
