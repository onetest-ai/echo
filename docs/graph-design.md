# The Brain's graph — entities and facts (plan)

> **Parked 2026-11-04.** Phases 1a/1b, the linker and call facts are on the `graph` branch (d5e349a), unmerged. The
> Brain is frozen while Echo's knowledge is tried as its own Obsidian vault, processed by an extension of the
> Obsidian plugin. What this plan learned (people are People's, tasks are Tasks', normalization in the pipeline, links
> only between organisations/products/initiatives, checks on samples) carries over.

Status: plan (2026-11-04), from a spike on a working data folder (below). Decisions:
**the Brain answers through three lanes over the same entities** — the **graph** (who and what, and how they are
connected), **retrieval** (what exactly was said, where), the **numbers** (figures from reports) — and **Tasks stays
apart** (operational work: tickets, status, due, owners; the Brain may point at a task, never holds its state).
The user's **practice** is the root; every organisation they work for is a **client** of it — their own company too,
with its threads like any other.

## Why

Today's Graph tab draws what the Brain never had. Its `graph_edges` hold only the taxonomy (`subclass_of`) and tags
(`about` section → topic, `mentions` section → term); the lines between topics are counted from section similarity and
mostly measure size (the biggest topics touch everything at 2–3× chance, the meaningful pairs are small ones at
15–30×); a topic's sources fan out on fixed rings. Brain's own README says the same: "the graph *is* the taxonomy as
vertices". The only entity-to-entity relation Brain had, `addressed_by` (problem → capability), Echo never ported.

What the user asks of a graph is different: find data, facts and the connections between them.

## How the lanes relate (decided 2026-11-04)

One example: a call where Acme says it is moving from Legacy CRM to Nimbus CRM, decision by the quarter's end.

- **Retrieval** keeps the text — the minutes' section at `[12:40]`, mail, posts, file pages — and is the only place
  that holds what was said; everything else points into it.
- **The taxonomy is topics only**: rubrics on sections ("CRM migration" › "Platform choice") — what a text is about.
  A topic is a shelf, not a thing: it has no `uses` or `decided`.
- **Terms are things, so they are the graph's entities**: Nimbus CRM ("Nimbus", heard as "nimbus"), Legacy CRM, Acme —
  names, synonyms, spellings. **One dictionary**: the graph's non-person entities. Transcription hints and mention
  finding read it as they read the taxonomy's terms today.
- **The graph** holds entities and facts between them (Acme `replaces` Legacy CRM with Nimbus CRM; Acme `decided` …),
  each fact quoting a retrieval section and linking to it; a fact inherits its section's topics, so "facts about
  Nimbus under CRM migration" works.
- **The numbers** hold figures per metric × entity × month from report workbooks; the entity there is the graph's
  entity (Acme in a mart is Acme in the graph). Neither the graph nor retrieval asserts a figure.
- **People** stays the source of truth for persons; the graph reads its directory, never a copy of its own.

```
        Taxonomy (topics)                         Numbers (marts)
   "what a piece of text is about"        "metric × entity × month"
              │ tags                                   │ same entity
              ▼                                        ▼
 Retrieval (sections) ◄──── quote / link ──── Graph: entities + facts
   "what was said"                              "who / what, how connected"
              ▲                                        ▲
              └──────── mentions ──────── its names = the dictionary (ex-terms)
```

### Terms move into the graph

What `taxonomy::Term` carries today goes with it, nothing lost:

| today (`taxonomy.rs`) | in the graph |
|---|---|
| name, `synonyms`, `variants` (spellings) | the entity's names (`kg_names`: name / synonym / spelling) |
| `group` (organisation, product, initiative, document, metric, method, concept, other), `kind` | the entity's kind |
| `layer` (the practice or a client) | the entity's scope (practice, or a client) |
| `threads` (pinned), `always` | pins on the entity; always-on stays a transcription setting |
| `learned` (subjects, people, calls — the vocabulary a minutes run learned) | the same scope rules on the entity; `vocab::for_call` reads them there |
| `description` | the entity's description |
| edits: set, split, rename, merge (`into_layer: shared`), scope, Move up | the same edits on entities (merge / split are also how name resolution is corrected) |
| mentions (`term_scan`, whole words, name or spelling) | `mentioned` links fact / section → entity |

The taxonomy file keeps only topics (with their synonyms, descriptions, layers, drift, moves). The Terms tab becomes
the graph's dictionary (the same search, groups, A–Z, panel); a term's topics and a topic's terms stay (sections they
share). Migration once, at the scheduler's start (as `vocab::migrate_learned` did), the old file kept beside it; readers
take either while it runs.

## The model

**Entities** — stable things with names:

| kind | from |
|---|---|
| practice (one), client, organisation | `clients.json` (orgs, accounts) |
| thread | the threads |
| person | People's directory (merged addresses, `canon`), call attendees, mail senders and recipients |
| product, initiative, document, method, metric, concept, place | the dictionary (the taxonomy's terms, moved here) and new names a reader finds |

**Facts** — long-term memory, what an agent may take as true:

| field | |
|---|---|
| `subject`, `relation`, `object` or `value` | entities (a value: a figure as said, a date, a short phrase) |
| `text` | one plain sentence |
| `quote` | ≤ 25 words, word for word from the source — checked, else the fact is dropped |
| `source`, `at` | the call and its moment, the mail, the post, the file and page — its link |
| `valid_from`, `valid_to`, `superseded_by` | when it became true; a newer fact on a single-valued relation closes the older one (kept, never deleted) |
| `read_by`, `reader` | the profile and reader version that produced it (strict routing, re-reads) |

Relations are a closed list, `related_to` the fallback: `works_for`, `role_in`, `member_of`, `part_of`, `client_of`,
`partner_of`, `vendor_of`, `competes_with`, `uses`, `owns`, `builds`, `wants`, `needs`, `problem_with`, `decided`,
`proposed`, `rejected`, `depends_on`, `replaces`, `responsible_for`, `interested_in`. Single-valued (a newer one
supersedes): `works_for`, `role_in`, `replaces`, `decided` on the same object.

**Structural edges** — what Echo already records, no model: thread → client, thread → counterpart organisations,
person → organisation, person → thread (a role there, or calls attended — **never** because they were copied on a
thread's mail: that is `on_mail_in`, shown apart), works for / reports to, organisation relations, term → thread
(pinned). Decisions from minutes are facts (what was decided, why); their follow-up work is Tasks'.

**Not in the graph**: tasks, stories, open questions or risks as work items, their state, due dates, owners. A fact
may name a task by link.

## Building it

1. **Skeleton** (deterministic, each Brain sync, only when its inputs changed): entities and structural edges from
   `clients.json`, the threads, People's directory and places, the calls' attendees, the taxonomy's terms.
2. **Facts from calls**: after a call's minutes, one no-tool job on the **call's** profile reads the minutes (the
   transcript is what quotes are checked against too) and returns entities + facts; the guard as for the other short
   jobs (`complete_text`, lean settings). Every decision with its reason; up to 60 facts a call.
3. **Facts from mail and posts**: the quarantine reader already turns each conversation or post into checked facts;
   its answer grows to subject–relation–object (same pass, same cache, `PROMPT_VERSION` bump). Only senders the user
   trusts, as now.
4. **People's facts** (the people pass) become graph facts about their person.
5. **Resolving names** (the step the spike showed matters most): exact name, synonym, spelling or address → a close
   spelling (≥ 0.9) → a first name unique among the thread's people → else a **new** entity, marked new. "Me", the
   user's name and its short forms → the user. One person under several addresses is one entity. The user merges
   and splits (as People does: `not_same`), and renames; a reader never merges two known entities on its own.

Each fact keeps its source's scopes (client, thread) and profile; the MCP reader's access narrows facts as it narrows
sections (`brain/access.rs`). Closed threads keep their facts (the archive), weighed as sections are.

## Storage

New tables in `knowledge.sqlite`, beside Brain's (whose `graph_*` stay the taxonomy, for kb compatibility):
`kg_entities` (id, kind, name, new, merged_into), `kg_names` (entity, name, kind: name | synonym | spelling | address),
`kg_facts` (the fields above), `kg_fact_entities` (fact, entity, role: subject | object | mentioned), `kg_edges`
(structural: a, rel, b, props, from). Rebuilt in place by the sync's delta (a source's facts replaced when it changed).

## Reading it

MCP tools on `echoctl brain mcp`, beside the retrieval ones (names final in phase 1):

- `find_entity(query, kind?)` — names, synonyms, spellings, addresses.
- `neighbors(entity, relation?, kind?)` — its **connections** (relation → whom or what, the facts behind each, with
  quotes and links), threads, people, organisation.
- `facts_about(entity, since?, as_of?)` — newest first; `as_of` hides what was superseded by then.
- `path(a, b)` — how two things are connected; well-connected hubs and the user cost more.
- `people(scope?, min_threads?)` — who is in which threads (roles, calls), apart from mail copies.

Their descriptions say when to use the graph (everything about an entity, who is involved where, connections,
across clients) and when retrieval (what exactly was said); answers cite titles and links, never ids. The
`<echo_brain>` advert lists the graph's counts.

The window's **Graph** tab becomes an entity view — one entity in the middle, its connections around it, a click on a
line shows its facts and quotes; the topic fan goes. Designed on the canvas first (separate step).

## Measured (spike, 2026-11-04, one working data folder)

226 sources (41 calls, 149 mails, 36 posts), 9 threads, 6 clients.

- Skeleton: ~540 people (153 from threads, the rest mail participants), 233 terms; ~1,900 flat facts already there
  (mail and post readers' facts, people facts, decisions).
- Facts from text: 64 sonnet jobs (40 calls' minutes, 24 batches of mail / post facts), $2.54 first pass, $1.48 for the
  calls again at 60 facts each; 1,032 facts kept, 5 dropped for a quote not found; 175 new entities (mostly
  organisations: prospects named in mail).
- Answers (same model, fresh session each, read-only; three arms — retrieval only / + skeleton / + facts from text):
  the user's blind judgment on knowledge questions, round 2: graph 5, retrieval 0, ties 2. An opus judge checking
  claims against the data, 15 questions after the fixes: wins 4 / 5 / 5 (one tie), mean 16.3 / 16.7 / 16.9 of 20,
  wrong claims 7 / 9 / 8, cost per answer the same (~$0.09). The judge agreed with the user's blind picks 6 times in
  12 — fit for catching wrong claims, not for preference. The model chose the graph in 14 of 15 questions.
- What the graph won: everything about one company or person, colleagues across clients, who else talked about a
  product, who is in several threads. What retrieval still wins: the details of one call's decisions (a section read
  whole beats extracted facts).
- What went wrong and is designed out above: mail recipients taken as thread members (4–6 wrong claims per arm until
  fixed); one person as two entities; ledger work in the graph (task questions belong to Tasks); ids shown to the user.

## Checking it

The spike's bench stays (outside the repo for now): the questions, the three arms, the judge. A change to the graph is
checked against it; a decision is taken on the user's blind judgment, the judge's wrong-claim count beside it.

## Phases

1. Terms move into the graph (one dictionary; transcription hints, mentions, the Terms tab and its edits read it);
   skeleton + `find_entity`, `neighbors`, `people`, `path` on the MCP server; resolution of names.
2. Facts from mail and posts (the quarantine reader's answer as subject–relation–object).
3. Facts from calls (a job after the minutes); decisions with reasons.
4. Time: `valid_from` / supersede, `as_of`; merge / split / rename of entities in the window.
5. The entity view replaces the Graph tab (canvas first).

(1b: one dictionary in use, one Taxonomy view, and the normalizer every pipeline step goes through — see the acceptance list.)

## Acceptance and functional tests, per phase (agreed 2026-11-04: stop before phase 5)

Every phase: its own branch; `cargo clippy`, `cargo test`, the Windows cross-check clean; a functional test on a copy of
the real data folder (never the live one), in the real window or a real Claude session; the bench where it applies;
committed without the design docs.

**1a — the graph reads the dictionary as it is**
- Built from the data alone (no model): every client, thread, person in People's directory and every call attendee is an
  entity; one person under several addresses is one entity; mail recipients are never thread members.
- `find_entity` finds by name, synonym, spelling, address; `neighbors` gives threads, organisation, people, structural
  edges; `people(min_threads: 2)` matches People's thread lists; `path` avoids hubs; ids never reach answers.
- Strict routing: a profile's reader sees only its threads' entities and edges (a test with two profiles).
- Rebuilt only when its inputs changed; a rebuild of the working store under 2 s.
- Functional: on a copy of the data folder, a Claude session in Echo's window answers the bench's people / connection
  questions through the graph, no wrong thread memberships; the bench's skeleton arm not worse than the spike's B′.

**1b — one dictionary in use (decided 2026-11-04: term records stay in the taxonomy versions underneath)**
- The graph is the dictionary everything reads: transcription hints, mention finding and the Terms tab go through the
  same records the graph is built from; a term that is a client, thread or person is shown and hinted as that entity.
- The user's decisions on identity live with the graph (`brain/graph/decisions.json`): **same as** (a term is this
  client / person / other term — its names join it) and **not the same** (never joined by name again). The build
  applies them; a removed entity's decision is dropped quietly.
- Unchanged where nothing was decided: the transcription hint for a call is the same before and after on ≥ 10 real
  calls; mentions per term the same.
- **One taxonomy view** (decided 2026-11-04: terms are a piece of the taxonomy, not a second view): the Brain's Topics
  and Terms tabs become one **Taxonomy** tab — the topic tree (practice, clients' topics under it), each topic with
  the terms that come up under it (sections they share), terms under no topic in their own group, the dictionary's
  search across topics and terms, one panel for either. The graph shows no dictionary of its own.
- A term's panel: what it is in the graph ("is the client EPAM Systems", Not the same), Same as… (search any
  entity: a client, organisation, person, thread or term); MCP `get_entity` shows the joined names.
- **Normalization is a step of the data pipeline** (decided 2026-11-04, not a phase of its own): one normalizer —
  the dictionary's names, synonyms, spellings and the user's same-as decisions — finds the entries a text names and
  gives their canonical names. Every step goes through it: mention finding and fact linking (the graph), embeddings
  (a section naming an entry by a synonym or spelling is embedded with the canonical name added — the stored text
  unchanged; sections whose additions changed are embedded again by the sync's delta), search queries (expanded with
  canonical names and synonyms), and the fact extraction of phases 2–3 (names resolved through it before anything new
  is made).
- Bench: questions naming things by a spelling or a synonym find what the canonical name finds; overall not worse.
- Functional: on a copy, through `echoctl window --serve` in a browser — the one tree with terms under their topics,
  search finding a term and a topic, a term made the same as a client and back, the hint and the graph following; a
  search by a misheard spelling finds the term's sections.

**Links: what they are, and what they are not** (decided 2026-11-04, after the first mail run: 13% of mail facts linked,
`related_to` and "X is responsible for Y" among them)
- People are People's: roles in threads, titles, whom someone works for, their organisation come from People (calls,
  Microsoft 365, the people pass with the user's yes) as structure — never a link the Brain extracts from text.
- A link joins organisations, products, initiatives, documents: uses, owns, builds, vendor_of, partner_of, competes_with,
  part_of, client_of, wants, needs, problem_with, decided, proposed, rejected, depends_on, replaces, interested_in. No
  `related_to` fallback: no relation from the list, no link.
- No new facts for links: a linker adds a link to a fact that exists (mail / post facts, call minutes' facts); a fact
  with no link stays plain text in retrieval.
- A new entity only when its name comes in two sources or more; once, it stays the fact's text.
- One linker for mail, posts and calls (over stored facts / minutes, cached by content): mail isn't read again from
  Microsoft 365 for it.
- Checked on a sample before anything wider: of 30 random links, ≥ 80% right and useful, ≤ 5% wrong.

**2 — facts from mail and posts**
- The reader returns subject–relation–object facts; each quote checked (else dropped); the same trust rules (only
  trusted senders read); its cache keyed by the new prompt version.
- Names resolve to existing entities first; new ones marked new; nothing merged on its own.
- Functional: on the copy, a full re-read in the background while the app runs; then `facts_about` a client and a
  prospect from mail answers with quotes and Outlook links; the bench's "everything about a company" questions not
  worse than the spike's C.

**3 — facts from calls**
- After a call's minutes, one job on the call's profile; never another profile; quotes checked against minutes and
  transcript; decisions with their reasons.
- Older calls read in the background only while nothing records; a failure is retried later, never blocks minutes.
- Functional: record (or replay) a short call on the copy, minutes written, its facts appear with links to the moment;
  the bench's decision questions: each decision of the test calls present.

**4 — time and entity edits**
- A newer fact on a single-valued relation closes the older (kept, `valid_to`); `as_of` hides what was superseded by
  then; `facts_about` newest first.
- Merge, split, rename an entity in the window; a merge moves facts and names; a split never merged again (`not_same`).
- Functional: on the copy, merge two records of one organisation and split a wrong merge in the window; the facts
  follow; a superseding fact (a person's new role) shows the old one as past.

## Open questions

- New entities: shown at once marked "new", or held for the user's yes (Inbox) above some count?
- The relation list: enough, or should a reader be allowed a free label kept as `related_to`?
- How far back to read on the first build (all calls, or the last 90 days)?
- Facts from transcripts directly, for calls without minutes?
