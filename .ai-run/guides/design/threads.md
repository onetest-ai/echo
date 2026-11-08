# Threads — design

Threads (the code, CLI, API and data folder say **project**) group calls, chats and mail under a client, and track the
work said in them in an evidence-checked ledger; People and Tasks are views over the same data.

Related: rules in `.ai-run/guides/development/threads-and-ledger.md`; plan, phases, the taska port, stages and the
stakeholder-map model in `docs/projects-design.md`. Recall indexing of mail, channels and folders is in
`.ai-run/guides/design/recall.md`; window mechanics in `.ai-run/guides/design/window.md`.

## Modules

| File | Role |
|---|---|
| `crates/echoctl/src/projects.rs` | Clients / organisations, projects, membership rules, suggestions, Found for you, files and lock |
| `crates/echoctl/src/ledger.rs` | The work ledger: events, fold, apply / verify, pins, evidence items, mail and chat sync, cross-thread links, call changes |
| `crates/echoctl/src/projectsync.rs` | The sync: scheduler, backfill, the guarded Claude runs (extract, verify, people) |
| `crates/echoctl/src/people.rs` | People facts, identity (merge / split), colleagues, Microsoft 365 lookups, people-pass suggestions, everyone view |
| `crates/echoctl/src/tasks.rs` | Tasks piles, Asks, the client page (`tasks::client`) |
| `crates/echoctl/src/guard.rs` | `RUN_MODE=project` scope; `PROJECT_STEPS` |
| `plugin/skills/project-sync/` | `SKILL.md`, `EXTRACT.md`, `DEPS.md`, `VERIFY.md`, `PEOPLE.md` — the sync's prompts |
| `crates/echoctl/src/window/threads.rs` | IPC `project {op, …}` → `window::project_op` |
| `crates/echoctl/ui/app/` | `ProjectArt` (`04-project-art.js`), `PJ_SYMBOLS` (`40-threads.js`), `RECORD_TEXT` (`41-work.js`), `OrgLayout`, `drawOrgLinks` (`44-org-chart.js`) (tests: `ui/projects.test.mjs`, `ui/org-chart.test.mjs`) |

## How it works

### Organisations, clients, colleagues
- `clients.json` holds **organisations** (name, other names, domains, internal, logo, parent, relations, groups). A
  **client** is an organisation marked `account` (the default, so entries from before organisations stayed clients).
  - `projects::clients()` = accounts: thread list, a thread's client, the client page, Recall layers and scopes, senders.
  - `projects::orgs()` = all: whose people are whose (`people::org_of`), org chart, map, rule matching, Found for you.
- A domain belongs to one organisation. One with threads under it stays an account. Removing one unlinks it everywhere.
- A thread's **counterparts** are its `orgs` (`OrgPlace`). Its `people` rule with no domains of its own takes its
  client's and its counterparts' domains.
- Made from Profile → Organisations, a person's unknown domain (Name this organisation → new counterpart), Found for
  you (an unknown domain proposes a counterpart, not a client, unless New client is picked), `echoctl org …`.
- **Colleague** (`people::Colleagues`, cached until `clients.json`, the directory or settings change): the user's own
  domain and internal organisations' domains; every address of a directory person who has one of those or is known in
  the tenant; on one invite, any address of a name that also comes with a colleague's address (a colleague invited on
  a client address). Used by rule matching (`CallFacts.colleagues`, `outside`), Found for you, transcript offers,
  the minutes' colleagues vs external, People's side.
- Chat messages with no user (bots, apps) are kept `bot: true` and are nobody in People.

### Membership (computed, never written into a call)
- Rules:
  - `series` — Graph `seriesMasterId` (`MeetingContext.series_id`), else the normalised subject;
  - `people` — at least half of the call's **outside** people from the rule's domains (else the client's);
  - `chat` — `<call>/chat.id` or the join link's chat;
  - `subject` — whole words.
- Plus pins and exclusions (call stamps), `since`, and a closed thread takes no call started after it closed.
  `Project.not_events` keeps a Teams-transcript offer's meeting out of the threads it was offered for.
- **Suggestions** (`projects::suggest`) are deterministic — half the call's people are the thread's, or the subject
  names it or a glossary term — and never assign. "Not this" (an exclusion) stops them.
- **Found for you** (`projects::found`, `found_accept` / `found_merge` / `found_reject`): calls of the last 90 days in
  no thread, two or more sharing a meeting series, one outside organisation (personal mail and colleagues aside) or
  one Teams chat → a candidate with rules, calls and client (existing, or new named from the domain).
- New thread from a call = that call pinned + its meeting series as the first rule.

### What threads feed elsewhere
- Minutes: `context.projects` = each thread's brief, notes for Claude, glossary, people and the highlights of its last
  5 calls (`calls::recap_from_notes`) — given as data, so the minutes guard needs no new reads.
- Transcription: `vocab::for_call` adds the glossaries of the meeting's threads; `vocab add` from a thread's call also
  adds to its glossary.
- A thread is drawn as its logo, else its client's (`icon: "client"`), else a `PJ_SYMBOLS` symbol, else an identicon
  from its id (`ProjectArt`). Profile → Look offers only the pattern (in its colour) or its own logo; a thread that
  picked a symbol or its client's logo keeps it, with "Use the pattern".
- Profile has no People or Terms rows (People has its tab, terms Knowledge → Dictionary); its Meetings row lists the
  thread's meeting series read only, × takes one out (`Project.not_series`; set a series on a meeting in the calendar).

### The work ledger
Model (taska's, see `docs/projects-design.md`): `ledger/events.jsonl` folded into Workstream (`E01 General` first) ›
Story (criteria) › Task (action / session / dependency, owners, due, client-owed) + decision / risk / question records.

- Stories and tasks: `description` (Markdown; agent ≤ 1200 chars via `create` / `revise`; user ≤ 4000, pinned) and
  `links` (`{title, url}`, https, ≤ 10).
- A record's substance is its `description`, named by kind (`RECORD_TEXT` in app.js):

  | Record | Holds | Open when |
  |---|---|---|
  | question | **Answer** | no answer and not `done` (`ledger::answered`, `record_open`) |
  | decision | **Decision** (what, options, why) + `decided_by` | — |
  | risk | **Response** + owner (`internal_owner`), `likelihood` / `impact` low·med·high | "no response" until it has one |

- An agent's answer or response is op `answer` (`text`, plus a risk's owner and levels), verified like a status change
  — never a `description` on a question or risk.
- `apply` answers with what it `created` (ids, titles, parents) and `touched`, so the next item and the dependency pass
  can refer to them.
- A new story or task whose title shares most words with an open one is offered as `suggest_merge` (To review), never
  merged by itself.
- Spoken dates are resolved against the evidence item's day (`ledger::resolve_due`).
- `verify_apply` gates status changes, criteria met, links and waits (only `supports` applies).
- The Work tab (the window's `work-status` block, tested by `ui/work-status.test.mjs`): the tree and the board give an
  task one status — done, then blocked (the user's own word), then waiting for someone, in progress, to do (questions
  and risks keep their own words in the tree: answered / open, handled / no response). A done card
  shows no waiting chip (a user's edit to done keeps `waiting_for` in the ledger; only a call's "done" clears it).
  Late is judged by the local date. The Owner filter lists the user once, "Me (name)", covering every owner here that
  `tasks::Me` takes for them (`tasks::my_names`, sent as the work route's `me`). Owner and Kind narrow every board
  column, Done included; Open only leaves Done alone.

### Evidence items
| Kind | Id | Content | Trust |
|---|---|---|---|
| call | `evidence/<stamp>.json` + `.md` | transcript turns `[hh:mm:ss] speaker: text`, chat, the user's notes | the thread's calls on its profile |
| mail | `mail-<sha8 of Internet message id>` | one turn: sender ("Me" for the user), local time, the new part of the text, Outlook `webLink` | hand-picked (`ledger::add_mail`) = the user's trust; from ✉ Mail sources = filtered (below) |
| chat | `chat-<sha8 of chat>-<day>` | one day's turns at local HH:MM:SS, files named | filtered (below) |

- A changed call or chat day is queued again by a `reprocess` event.
- **Who may speak** (mail and chats): only the user, colleagues and senders trusted for the thread's client (Recall's
  `senders` list); a trusted outside sender's mail only with DMARC / compauth passing. Others wait in Needs you (asked
  there even with Recall off). No organisation domains known → nothing read. An author not looked up yet holds the
  day.
- Trusting a sender (or reading their unauthenticated mail anyway) re-reads that client's threads' mail and chats
  from the start (`ledger::reread_sources`).
- **Mail sources** (`ledger::mail_sync`, `sync --mail`): the thread's ✉ Mail sources (`domain:` / `person:`), first
  read 30 days, later 7, ≤ 200 a read; state in `ledger/mails.json`.
- **Meeting chat between calls** (`ledger::chat_sync`, `teams_chat` sources, `sync --chats`): `graph::chat_history`
  (≤ 2000 newest; first read 30 days, later 7; days rebuilt); messages within ±2 min of this thread's calls on that
  chat are left out (the call's item has them); a day is written once the chat has been quiet for
  `projectsync::SETTLE`; state in `ledger/chats.json`.
- Both read whether or not Recall is on (but need the `brain` cargo feature).

### The sync (`projectsync::run`)
1. `ledger::sync` builds evidence items (a new thread backfills its last 20 calls, `BACKFILL`), reads chats and mail.
2. Claude run 1, **extract** (+ dependencies): `EXTRACT.md`, `DEPS.md`.
3. Claude run 2, **verify**, in a fresh process: `VERIFY.md`.
4. Claude run 3, **people pass**: `PEOPLE.md` (below).
- On the thread's AI profile and model; ≤ 10 items a run; state `ledger/sync.json`, log `runs/<run>/log.txt`.
- Guard `RUN_MODE=project`: read the ledger, the skill folder and the session's own tool-results (a large `context` /
  `people-context` Claude Code saved to a file); write only `.json` in `runs/<run>/`; Bash only `echoctl project
  ledger <this id> <PROJECT_STEPS>`. The CLI checks `ECHOCTL_RUN_PROJECT` too and keeps `verify-prep --out` in the
  run folder.
- Scheduler (`projectsync::due`, every 5 min, `config.project_sync`, default on): open threads with a call ended
  ≥ 15 min ago not read yet, or a chat not checked for 30 min / a chat day unprocessed. A failed sync waits an hour.

### People
- Facts are **recomputed from the calls each time** (invitees, organisers, named speakers and their share of talk,
  chat posters, calls attended, first / last seen, tasks owed / late / waiting on them). **new** = first seen ≤ 14
  days; **quiet** = not seen > 6 weeks while calls went on.
- Names read as people read them (`people::display`: "(Partner)", "(external)", "[EXT]" dropped, "Last, First" turned
  round, accents folded for comparison).
- Identity: the same cleaned full name merges by itself (other address in `also`); a shared first + last name is only
  suggested (`maybe_same`); the user merges (`people::merge`) or splits (`people::split` → `not_same`, never merged
  again). A person's addresses are the calls' and the directory's (an address merged in that no call shows is listed
  too, so its "Not the same" can undo the merge; a merge records the names and addresses it brought in, and splitting
  such an address takes those names away again). The user is left out.
- The user sets: the directory `people.json` (each field with source and history), placements in a thread (`Member`:
  role, stance, influence / interest 1–3, `governance`, `group`, `works_for`), organisations as stakeholders
  (`OrgPlace`), an organisation's parent, typed relations (`RELATIONS`) and groups.
- Org chart = who does what **in this engagement**, not reporting lines (rarely known). Per organisation: a governance
  band, then a column per workstream (`Member.group`, else the directory's group, else read from roles — "<X> stream
  lead", a shared "<X>:" prefix — shown "from roles", never saved unless Keep these columns), lead on top, the rest by
  `people::seniority`. The `works_for_hint` (who handed tasks over, else stories' owner) stays within one stream and
  never comes from governance, PMs or partners (`people::coordinates`, `role_stream`).
- A role keeps `role_by` (you = pinned, never proposed over; echo), `role_at`, `role_from`.
- **Microsoft 365** (`graph::person`, `people::refresh_m365`): the People API gives colleagues' title, department,
  company, office, phone. Asked when a People tab opens, in the background, one refresh at a time, each person at most
  weekly, ≤ 40 a run. A value lands with source `m365` unless the user set that field — then it is a question (Use
  theirs / Keep mine; a kept value isn't asked again).
- **People pass**: reads calls not read yet (`ledger/people-read-2.json`, ≤ 10 a run, also when the work has nothing
  new) with `people-context` and proposes role, stance, influence, interest, title, group, reports to, left,
  organisation relationships, and `person` (named but never invited, one proposal per name). `people-apply` →
  `people::apply_suggestions` checks the quote is one span of that call (`ledger::quote_in`), the target is known, the
  value valid, new, not turned down before → `people-suggestions.json`, pending. The user answers Yes / No / Change…
  (`people::decide`; a correction is checked as a proposal is, state `corrected`).
- The same pass writes **facts** (one line ≤ 240 chars, quote checked, applied at once to `Person.facts`, deduped by
  person + item + quote, ≤ 300) and **who they are** (`about`, ≤ 800 chars, source `echo`; the user's own, ≤ 2000, is
  pinned and never rewritten).
- Per-person view adds threads, `touches` (distinct sources) and `with` (≤ 3 people sharing ≥ 2 calls), rebuilt only
  when inputs' file times change or every 10 min (`people::view_cached`).
- `people::everyone` (Recall's People view): only people with ≥ 2 sources, a placement or the user's description.
  MCP `find_people` / `get_person` see only the threads on the reader's profile (`Access::people_threads`).

### Tasks and Asks (`tasks.rs`)
Every open thread's open tasks in piles:

| Pile | Rule |
|---|---|
| I owe | owner = the transcripts' "Me", or `config.me`'s name / first name / email; a blocked one stays, marked |
| Owed to me | someone else owns it and `delegated_by` is me, else the speaker whose words created it is me (hand-filed tasks need the field) |
| Waiting for others | each open task mine wait on that isn't mine, plus a card per person a task of mine waits for (`waiting_for`) |
| Nobody named | no owner |
| Others | other people's work I neither handed over nor wait on |

- Order: overdue, then due date, then longest quiet (days since the last word in the ledger).
- **Asks**, by person: owed-to-me tasks late, due within 3 days or quiet ≥ 5 days, plus what my work waits on them for
  (asked once); ≤ 2 per story, ≤ 15 in all; with each person's next meeting in 7 days.

### One piece of work in two threads
- Link kinds: `same` (one piece of work; `master: here | there`) or `depends_on` (the other side records `blocks`).
  Written as the user's edit in **both** ledgers (`ledger::link_threads` / `unlink_threads`); undo of either side
  unlinks both; both items must be open.
- Read through `ledger::fold_linked`: the master takes verified status changes its mirrors' threads heard (newer than
  its own; history marked `from`); a mirror shows its master's state, owners, due and last history (`mirror_of`) but
  keeps its own evidence; `also_in`, `xwaits`.
- Tasks shows a pair once (the master's card). Search for a target: `ledger::search_open` (title words, no model).
  EXTRACT sees mirrors as known (`also_tracked_in`) so it doesn't re-create them.

### What this call changed
- `ledger::call_changes`: per thread the call is in, every history entry whose evidence is this call, in the order
  said, with words, speaker, time and weak / to check / kept-yours; mentions and entries the verifier called wrong
  are hidden. A thread that hasn't read the call says so.
- Shown as What moved beside the call's minutes, the user's own tasks first with who does each (`taskmodes`); the
  call's Actions tab and its action-item ↔ task matcher are gone (Nov 2026): action items became tasks, tasks are
  what gets closed. `ledger::open_tasks_of_call` gives a call's still-open tasks (prep's Last time, Still to do).

### Client page (`tasks::client`)
Overview (the user's `projects/clients/<id>.md` + a card per thread: stage, calls, stories done, open tasks by pile,
last 3 decisions, open risks — without a response first — and questions), Calls, Tasks, People, Profile. Calls, Tasks and
People show their count, none when it is zero (`clientTabs`, `client-tabs.test.mjs`).

## Decisions

| Decision | Why | Where |
|---|---|---|
| Membership is computed from rules, pins and exclusions, never stored in a call | A call can be in several threads; rules change; calls stay untouched | `projects::calls_of`, `projects::suggest` |
| Suggestions never assign | A model or heuristic must not move a call into a thread | `projects::suggest` |
| Colleagues don't count toward `people` rules | Internal people attend every client's calls | `projects::CallFacts`, `people::Colleagues` |
| A client is an organisation marked `account` | Partners / vendors need domains of their own without becoming clients | `projects::clients`, `projects::orgs` |
| Every proposal carries a quote that must be one span of its item; state changes need `supports` | Agents propose, the script disposes (taska) | `ledger::apply`, `ledger::quote_in`, `ledger::verify_apply` |
| Answers / responses go through op `answer`, verified; never a `description` | Closing a question is a state change and must be evidenced | `ledger::apply` |
| User edits pin fields and are undoable | An agent's later evidence must never overwrite the user | `ledger::human_event` |
| Merges are suggested, never automatic | Near-duplicate titles are often different work | `suggest_merge` in `ledger::apply` |
| Verify runs in a fresh process | The verifier must not share the extractor's context | `projectsync::run` |
| Mail and chat only from the user, colleagues and trusted, authenticated senders | Outside text reaching a tool-using run could carry instructions | `ledger::mail_sync`, `ledger::chat_sync` |
| Chat messages near a call are left out of chat items | The call's item already has them (no double evidence) | `ledger::chat_sync` |
| A chat day is written only after `SETTLE` quiet | Today's day grows; an item that changed is processed again | `projectsync::SETTLE` |
| People suggestions have no model verifier: the quote check plus the user's yes | Nothing about a person changes without the user | `people::apply_suggestions`, `people::decide` |
| A field the user set is never overwritten by Microsoft 365 — it becomes a question | Precedence user > M365 > call | `people::refresh_m365` |
| Org chart columns are workstreams, not reporting lines; role-read columns not saved unasked | Reporting lines are rarely known; the token lacks `User.Read.All` | `people::coordinates`, `OrgLayout` |
| A user-set role is never proposed over | Pinned like ledger fields | `role_by` in `people.rs` |
| Cross-thread links are written in both ledgers and unlink together | Either side's undo must not leave a dangling half | `ledger::link_threads` |
| Thread and client changes go through one lock and atomic writes | Several writers (app, CLI, window) | `projects::update`, `projects::update_client` |

## Files and state

| Path | What | Who writes |
|---|---|---|
| `<home>/projects/clients.json` | organisations / clients | user (window, `echoctl org|client`), Found for you |
| `<home>/projects/clients/<id>.<ext>`, `<id>.md` | client logo, client description | user |
| `<home>/projects/people.json` | people directory (fields with source + history, facts, about) | user, M365 refresh, people pass |
| `<home>/projects/found.json` | rejected Found-for-you keys | `projects::found_reject` |
| `<home>/projects/.lock` | lock for all project files | `projects::update*` |
| `<home>/projects/<id>/profile.json` | stage, client, rules, pins, exclusions, sources, members, board, AI profile | user, `projects::update` |
| `<home>/projects/<id>/brief.md`, `logo.*` (≤ 512 KB) | brief, logo | user |
| `<home>/projects/<id>/people-suggestions.json` | pending people-pass proposals and answers | people pass, user |
| `<home>/projects/<id>/ledger/events.jsonl` | append-only ledger (one write per batch) | `ledger::apply`, `human_event` |
| `<home>/projects/<id>/ledger/evidence/` | evidence items (`.json` + `.md`) | `ledger::sync`, `add_mail`, `mail_sync`, `chat_sync` |
| `<home>/projects/<id>/ledger/{sync,mails,chats,people-read-2}.json` | sync state, per-source read state, people-pass cursor | `projectsync`, `ledger` |
| `<home>/projects/<id>/ledger/runs/<run>/` | proposals, verdicts, `log.txt` | the guarded runs |

## Known gaps
- Reporting lines and managers can't come from Microsoft 365 (`User.Read(Basic).All` not in the token): only from
  calls and the user.
