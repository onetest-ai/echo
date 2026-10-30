# Echo projects — plan

Status (2026-10-29): phase 1 in progress — clients, projects, stages, membership, the Projects view (Brief, Calls,
Profile), call chips and suggestions, project icons, Found for you, glossary → vocabulary, project context in the
minutes are built; phase 2 — the ledger, the guarded two-run sync (extract, then verify) and the Work tab —
is built and was run on real calls; the Tasks activity is built (without people's priorities: no people directory yet); so is a call's "What this call changed" with
action items linked to their tasks (matched deterministically, shown once); the people directory and live notes'
project context are next. Boards: the "Projects" row of the Echo
Workbench canvas.

## Goal

People work on a few projects at once: client engagements, internal initiatives, hiring. Calls, chats and later mail
belong to them, often to more than one. Today Echo knows only calls, and approximates "project" three times over:

- **Vocabulary:** learned terms are scoped to a meeting's subject and people (`vocab.rs`, since one project's terms
  leaked into every call on 2026-10-13).
- **Earlier context:** a minutes run reads only the previous minutes of the *same recurring meeting*
  (`calls::previous_subject`). Decisions from a related call with another title are invisible.
- **Chat:** only the meeting's own chat is captured. The project's ongoing chat, where half the decisions happen, is
  never read.

A project is the scope all of these hang on, and the place where work said in calls is tracked across them.

## Decisions so far

| Decision | Answer |
|---|---|
| Core or plugin | **Core.** Projects, the work ledger and tasks are Echo's own. |
| Calls per project | A call can belong to **several** projects. |
| Client level | **Yes**: a client (name, other names, mail domains) above its projects; *Internal* for one's own company. |
| Mail | Needed once tasks and the brain integrate (threads continue what calls start). Built later, opted in separately. |
| Local folders | A project's folders become sources and part of the brain. Built later. |
| Task tracking | **Port taska** into Echo now (Rust, no Python), as the engine behind projects. |
| Ideas from an assistant prototype | Concepts only, in our own words: the repository has no license. Never its code or prompts. |
| Verified changes | **Apply by themselves**, each on the Timeline with Undo (an undone change isn't made again; pinned fields are never touched). |
| To review | **Inside Projects**: a row with a count on top of the project list, a badge on the activity; each project's own items also in its tabs. |
| Backfill | **Automatic, the last 20 calls** of a new project, oldest first, stoppable; older ones on request. A call is evidence by its transcript (plus its minutes, chat and notes when there are any), so calls without minutes are processed too. |
| AI for a project | **Like the minutes:** each project picks its AI profile (Work, Local, …; new ones get the default) and its model (default sonnet). It is what Claude runs on for the project's jobs, whatever profile a call was recorded on. |

## Where each piece comes from

### Already in Echo (reused as is)
- Calls with timed transcripts, speakers, chat, frames, minutes and action items (`actions.json`, stable ids).
- The guarded headless Claude run (`guard.rs`, `notes.rs`): only listed `echoctl` commands, reads scoped to named
  folders, the call's AI profile, the `system` / `claude` / `local` routing. Project jobs reuse it with a new scope.
- Microsoft Graph client (`graph.rs`): calendar, meeting and call chats. New scopes are added only when a phase needs
  them.
- Claude sessions in the window (`terminal.rs`): a guarded **Claude · <project>** session is a third kind beside
  "this call" and "Echo folder".
- The workbench registry (activities, panels, call tabs, ⌘K palette).

### From taska (onetest-ai/applied_skills `bundles/taska`, ported to Rust)
The engine. Taken nearly whole, because it already solves the hard parts:

- **An append-only event log per project; the work tree is a fold of it.** Epic → Story → Task.
  - Stories have 2–6 observable acceptance criteria.
  - Tasks have one owner and a type: `session` (a meeting that must happen), `action`, `dependency` (waiting on
    something; `client_owed` + `client_owner` when the client owes it).
  - Risks and decisions are records under an epic or story.
- **Evidence items:** every source normalised into one item (`.json` + readable `.md`), deduplicated across sources.
- **Agents propose, the script disposes.** Proposals: `create`, `status` (held, done, progress, blocked, cancelled,
  reopen, due, delayed, reassign, mention), `revise`, `ac_met`, `link`, `waiting`, `suggest_merge`, `suggest_move`,
  `note`. Each carries a verbatim quote, and the script refuses:
  - a quote that isn't in the item;
  - a broken level rule (a task needs a story and an owner, a date or the type session/dependency);
  - a change to a node before it existed.
- **Three passes per batch:** extract (one item at a time, oldest first, each seeing the tree the last one left),
  dependencies (the whole tree in view: links across stories, taska's biggest measured gap), verify (a different
  agent; only `supports` applies status changes, criteria met, links and waits).
- **Rules in the prompts:** a promise is not progress; presupposition counts ("how was the billing session?" ⇒ held);
  a recording of the session is proof it was held; "I" = the speaker, "we" = nobody.
- **Human edits pin fields.** An agent's later evidence lands in the history but never overwrites them. Every human
  event can be undone.
- **Computed, never judged:** quiet items (no word for 3 meetings and 2 days), overdue, presumed-held sessions,
  done-to-confirm, and a ranked **Asks** list (at most 15, 2 per story) grouped by person; a cited **prep pack** for
  the next status meeting.
- **Its tests** (~20 files: dates, provenance, robustness, edge cases) become the Rust tests: they are the
  specification.

What changes in the port:

| taska | Echo |
|---|---|
| `taska.toml` + `/taska:onboard` | The project's **Profile** and **Found for you** (below). No setup interview. |
| `meetings_folder`, `msgraph_transcripts` sources | Echo's own **calls** are the meeting evidence: better transcripts, speakers, chat and moments. |
| `msgraph_calendar` | Echo's calendar (`graph.rs`). A scheduled / moved / cancelled event is evidence for a `session` task. |
| `files_folder`, `code_host` | Phase 4 sources. |
| `workbench.html` + local server on 8766 | The window's **Work** tab and **Tasks** activity. No server, no port. |
| Python CLI `taska.py` | `echoctl project …` (Rust, `project/` module), one JSON answer per command, as `context` is. |
| `/taska:run` subagents in the user's session | One guarded **project sync** run Echo starts itself, on the project's AI profile. |
| Whisper via ffmpeg for recordings | Not needed: Echo transcribes on the device already. |
| (no such record) | A **question** record: our minutes and the Brief track open questions. |

### From an assistant prototype (ideas only)
- **Clients above projects**, with mail domains that decide who is "theirs".
- **A stage** the sync is told (assistant has two: Prospect, Engagement; ours are richer, see *Stages* below).
- **Discovery instead of setup:** projects are *found* from what you already worked on, and you Accept, Merge (drag one
  onto another), Move (to another client) or Reject. Rejected ones are never suggested again.
- **Records with their words and source, and verdicts on them:** Right, Fix, Done, Dismiss (with why: not about this
  project, trivial, duplicate, wrong). "You judged 18 · 15 worth keeping" shows how well the sync works.
- **Proposed for the profile:** people, terms and domains the sync noticed, to add or decline.
- **People who matter:** a priority per person (critical, high, normal, irrelevant) that ranks asks and tasks.
- **Tasks by owner:** I owe · Owed to me · Waiting for others · Nobody named.
- **A meeting brief:** who comes, what changed since the series last met, what is open, what to raise.
- **Sessions per project:** Claude conversations kept with the project.

### New in Echo
- **Calls in several projects**, by deterministic rules (meeting series, a client's people — at least half of a call's
  *outside* people, colleagues don't count —, the chat a call came from, a subject) plus pins and exclusions. A model may *suggest* a project for a call, never assign it.
- **The glossary scopes vocabulary.** A project's terms hint transcription on its calls; terms the minutes learn go to
  the project's glossary when the call is in one, else to the scoped list as today.
- **Project context in the minutes:** `echoctl context` gives the call's projects (brief, open questions, recent
  decisions, glossary), and the guard lets the run read the minutes of the project's recent calls, not only the same
  series.
- **Action items linked to tasks** (the rules decided in `plugins-design.md`, now core): an action item is what was
  said in one call; a task is the work tracked across calls. Done shows on both sides.
- **Claude · <project>:** a guarded session that reads the project's calls, chats and ledger.
- **Live notes** know the call's projects (resolved at recording start), so the running summary uses its terms.

## The model

```
Client ── Project ─┬─ Profile: stage, since, rules, sources, people, glossary, workstreams, notes for Claude
 (domains)         ├─ Brief (yours, Markdown)
                   ├─ Ledger: events.jsonl → tree  Workstream (epic) › Story › Task
                   │                               + Decision · Risk · Question records
                   └─ Evidence: calls, calendar events, chat days; later mail threads, folders, PRs
Call ── many-to-many ── Project        Action item (one call) ── many-to-one ── Task (a project)
Person (email) ── side from the client's domains, priority set by the user
```

- **Workstreams** are taska's epics. A new project starts with one, *General*; the sync may create an *Off-plan* one
  when work fits none (taska's rule: a finding, not a failure). The user adds, renames and merges them.
### Stages
Three groups, each with its stages. The group decides what Echo does; the stage is for the user and the sync.

| Group | Stage | Meaning | What changes |
|---|---|---|---|
| **Pursuit** | Lead | An opportunity or an idea, nothing committed | The sync tracks interest, asks and next steps; no delivery tree yet |
| | Proposal | An RFP, a proposal or a business case being written | Deliverables of the proposal become tasks; the client's questions become asks |
| | Negotiation | Scope, price, contract or approval being agreed | Decisions and open terms are what matter; a signature or "approved" is proposed as the move to Kickoff |
| **Delivery** | Kickoff | Won or approved; team, access and plan being set up | Dependencies (access, people, data) are what the sync looks for first |
| | Active | The work itself | Everything |
| | On hold | Paused by someone's decision | No quiet flags or asks; calls still join it; a call about it proposes Active again |
| | Wrap-up | Handover, final reports, the last invoice | Open items are listed as what's left to close |
| **Closed** | Done | Finished | Archived: no suggestions, no sync; rules stop matching new calls |
| | Lost | A pursuit not won (or an internal one not approved) | As Done; the reason is kept |
| | Cancelled | Stopped during delivery | As Done; the reason is kept |

- Internal initiatives use the same stages: *Lead* is an idea, *Negotiation* is approval.
- A stage change is the user's. The sync may only **propose** one in To review, with its quote ("the SOW is signed" →
  Kickoff), and every change is a dated event in the ledger, so the Timeline shows when it moved.
- Closed projects can be reopened (back to the stage they left).

- **People** are one directory for the data folder, keyed by email, with aliases (Teams display names, speaker
  labels). Side (client, internal, external) comes from the client domains; priority is the user's.

### Files
```
<home>/projects/
  clients.json                    [{id, name, aliases, domains, internal}]
  people.json                     [{email, name, aliases, priority, title}]
  <slug>/
    brief.md                      the user's brief
    profile.json                  id, client, color, stage, since, rules, pins, exclusions, sources, terms, notes
    ledger/events.jsonl           append-only (taska's format, plus Echo's ids)
    ledger/evidence/<id>.json|.md
    ledger/runs/<run>/            proposals, fixes, verify packets and verdicts, log
    review.json                   what waits for the user (below)
```
Calls are never written by project code: membership is computed from the rules (`echoctl project calls`), and the
projects predicted at recording start go into `meta.json` `projects` only as a hint for live notes and vocabulary.
Every file is written atomically under a lock per project, as `meta.json` is.

## Flows

1. **Finding projects** (`Found for you`). Deterministic first: clusters of calls by meeting series, by a client's
   people, by the chat they came from. A model run names them and proposes stage and workstreams. Accept creates the
   project with those rules; Merge, Move and Reject as in assistant.
2. **New project by hand:** one inline row (name, optional client). Echo fills the rules from the calls that match the
   name and its people; the Profile tab refines them, showing live how many calls match.
3. **A call ends:** transcript → minutes (as today, now with project context) → for each of its projects, the call
   becomes an evidence item (its transcript, plus minutes, chat and the user's notes when present) → the **project sync** runs (extract → dependencies → verify) → the ledger folds → the
   Brief, Work tab and Asks update. Action items the sync matched get their task link.
4. **Batching:** a project's sync runs after a call's minutes when the switch is on, or in the nightly pass, at most N
   items per run (taska's `--due`), oldest first. Creating a project backfills its last 20 matched calls by itself
   (the count and an estimate shown, Stop at any time); older calls only on request.
5. **Calendar:** each sync also reads the project's events since the last one: scheduled, moved or cancelled sessions.
6. **Chats (phase 4):** a chat source becomes one evidence item per active day.
7. **Human edits** in the window go through `echoctl project edit` as human events: pinned, undoable.
8. **Before a meeting:** the Calendar's popover and the Brief show what changed since the series last met, what is
   open, what to raise (computed); **Sum up** asks Claude for a short brief (a job, on demand).

### What applies by itself, and what waits for the user
taska applies a verified change at once; `plugins-design.md` (round 3) said every inferred change waits in Review.
With three passes per call, review of every change won't be read. So:
- **Applies by itself, shown in the timeline with Undo:** verified status, dates, criteria met, links, waits, new
  stories and tasks with a quote.
- **Waits in To review:** projects found, profile proposals, `suggest_merge` / `suggest_move`, done-to-confirm (a
  `done` or `held` the verifier called weak), a call suggested for a project, a new client.
- **Verdicts on records** (Right, Fix, Dismiss with why) are the user's correction and the sync's measure.

## In the window

- **Projects** activity (⌘3). The list is grouped by client, then *Internal*, then *Not in a project* (calls this
  week, with suggestions); **To review** with a count sits on top. A project's tabs:
  - **Brief:** what it is (the user's), where it stands (the last decisions and moves, each with its source), Open, To
    raise at the next meeting.
  - **Work:** the tree and a board (status columns), filters by kind (stories, tasks, decisions, risks, questions),
    owner and workstream; each node with its history of quotes and sources; verdicts on records.
  - **Timeline:** calls, calendar changes, chat days, later mail and files, interleaved, with what each changed.
  - **People:** the project's people with side, role and priority.
  - **Profile:** stage, client, since, calls (rules, pins, exclusions), sources, people, terms, notes for Claude.
    Saved as it changes. Proposed additions sit on top.
- **Tasks** activity: across projects, I owe · Owed to me · Waiting for others · Nobody named, ranked by due date and
  the people's priority; Asks by person.
- **Calls:** project chips in the list and the header, a project filter, the suggestion bar (Add · Not this), the
  `+ Project` picker (several; *New project from this call*; *Always for this meeting series*). An action item linked
  to a task shows "✓ in <task>".
- **Right panel:** *Claude · <project>* as an icon tab next to My notes; no button for it in the header.
- **⌘K:** a project as a target (its tabs, its Claude session, sync now), and Go to any project.

## Sources beyond calls: channels, mail and folders (next, with the Brain)
Decided 2026-10-30:
- **Teams channels are thread-level**: a thread's Profile picks its channels; a client shows the aggregate of its
  threads' channels.
- **Mail** joins a thread as calls do (a DL, the client's domains, subject words, a conversation).
- **Local folders are thread sources** too: the folders where the thread's documents live (critical for the Brain).
- **Nothing is copied into Echo.** Messages, mail and documents stay where they are; every fact points to the exact
  Teams message, Outlook mail or file (its own link). Only the Brain's index holds the text it needs to search (as
  Brain's chunks do today); an item gone at the source leaves the index, and answers link to the source.
- They are Brain sources (facts beside the calls) and feed the stakeholder facts (who posts, who is on the thread).
  Folders bring Brain's document lanes along: PDF / Word / PowerPoint text, page images read by a model, spreadsheets
  into the numbers lane.
- Measured 2026-10-30: the CodeMie app's token already carries `ChannelMessage.Read.All`, `Team.ReadBasic.All`,
  `Mail.Read`, `Group.Read.All` at Contoso — no new sign-in.

## People and organisations (stakeholder maps)

Decided 2026-10-30; designed on the canvas (the "People" row). A **People** tab on every thread and client, three views
of the same data: **Map** (influence × interest: manage closely, keep satisfied, keep informed, monitor; stance as
colour), **Org chart** (organisations as nodes with their relationships, groups inside, people in groups, "reports
to" lines), **List** (contacts: role, threads, stance, calls, last seen, mail / Teams). A person's details sit in the
right panel. Organisations are stakeholders too: placed on the map with a stance like people.

### The model
- **Person** (`<home>/projects/people.json`, one directory for all threads): id, name, emails (the key), aliases (name
  variants, Teams display names), organisation (a client id) and group, title, phones, reports to (a person), how much
  they matter to the user (critical, high, normal, irrelevant), notes; each field remembers its **source**.
- **Organisation** = a client (`clients.json`), now with a parent, typed relationships to others (parent / subsidiary,
  partner, vendor, delivers for, competitor) and named groups. A partner gets its own domains, so its people are
  recognised as its own, not just "outside".
- **In a thread** (`profile.json` `people`): who takes part, with their role in it, stance (champion, supporter,
  neutral, sceptic, blocker), influence and interest (low / mid / high); the same for organisations.

### Discovery: facts and suggestions
Two layers. **Facts** need no judgment and apply as they come; **suggestions** come from Claude with the words or the
document behind them and wait for the user's yes, verified by a separate pass as the work ledger's are.

| Source | Facts | Suggestions | When |
|---|---|---|---|
| Calls | invitees and organiser with emails; speakers (named from Teams' tiles) and their share of the talking; chat senders; calls attended, last seen; tasks owned, what waits on them | role, stance, influence, reports to, group, relationships between organisations — each with its quote | now |
| Calendar | who meets whom and how often (unrecorded meetings too), who organises | interest (from cadence) | now |
| Teams chats | members, posters, @mentions | as for calls, quoting the message | now (meeting chats) |
| Entra / Microsoft 365 directory | title, department, company, office, phones (the People API: colleagues only) | — | built; managers need `User.Read.All`, which the CodeMie app isn't granted at Contoso |
| Mail | who is on which threads (To/CC → groups, DLs); signatures (title, phone, company) | roles, relationships, who decides | phase 4 (`Mail.Read`, its own opt-in) |
| Documents (local folders, SharePoint/OneDrive) | parties of an SOW or contract, RACI tables | structure, owners, relationships — citing the document and page | with the brain |

- **Organisations** are found as threads are: domains in no client grouped into candidates (accept, merge, reject);
  parent / subsidiary from shared domain roots, Entra's company name, later signatures and contracts; "partner",
  "integrates for" from who shows up together and what was said.
- **Identity:** one person may have addresses at several organisations (a partner or guest account beside their
  own): the directory keeps all of a person's addresses. Display names are cleaned ("Ortiz, Karen (Partner)" →
  Karen Ortiz); the same cleaned name merges; a shared first and last name (in names or address local parts) is
  suggested; the user merges or splits, and a split is never merged again. Name variants, Teams names and Entra ids attach to it; a "Speaker N" gets a name
  from the tiles or the user. Unsure merges go to To review, never silently.
- **Precedence:** the user > Microsoft 365 > a document or signature > a suggestion from a call. Every value keeps its
  source; a conflict ("IT Manager" in Entra, "Head of IT" in a newer signature) becomes a question, never an overwrite.
- **When:** facts are recomputed from the files as calls, invites and chats come (no model); suggestions are a third
  pass of the thread's sync (after extract and verify), on its AI profile, landing on the People tab and in To review.
- External people are mostly not in the user's directory: their structure comes from calls, mail and documents.

### Keeping it current (not a one-off discovery)
People join and leave a client, groups are reorganised, roles and stances shift; the maps follow:
- **Facts are recomputed from the files**, never snapshotted: a new invitee, speaker or chat poster appears by itself.
- **First and last seen** per thread come from its calls (earliest, latest): people who first appeared in the last
  14 days are marked **new**; someone not seen for 6 weeks while the thread's calls went on gets a question —
  "Still in Aurora?" (never removed by itself).
- **Microsoft 365 is read again weekly**: a changed title, department or manager is recorded with its date and source;
  one that contradicts what the user set becomes a question.
- **The sync's people pass runs with every sync**: it proposes changes (a new role, a stance that moved, someone who
  moved group or left, a new relationship between organisations), not only additions — each with its quote.
- **Every person and organisation keeps a history** of changes (what, when, from which source), shown in the details
  panel; the People tab counts what is new or changed since the user last looked.

### Build order
1. The directory and organisations; facts from calls, calendar and chats.
2. The People tab: Map, List, the person in the right panel; edits.
3. The org chart: organisations, relationships, groups.
4. Entra lookups (titles, departments; managers where consented).
5. The sync's people pass: suggestions with quotes (checked word for word; the user's yes is the verifier).

All five built on 2026-10-30.

## Guarding the jobs
- **Project sync:** `ECHOCTL_RUN_MODE=project`, scope = the project folder and its evidence. Allowed:
  - Read in the evidence files and the project folder;
  - Write only `ledger/runs/<run>/*.json`;
  - Bash only `echoctl project <context|pending|apply|done|verify-prep|verify-apply>` for this project.

  Its CLI refuses any other project. Evidence is data, never instructions (taska's rule, already ours for
  transcripts).
- **Claude · <project>:** like *this call*, in ask mode: reads, `echoctl project context|status|asks`, no edits.
- Both run on the project's AI profile and model (Profile fields; default = the default profile, model sonnet), with the
  same routing, sign-in and policy checks as the minutes. It reads the project's calls whatever profile each was recorded on.

## Phases

1. **Clients and projects.**
   - Files, `echoctl project` / `client` commands, membership (rules, pins, exclusions), the people directory.
   - Projects activity: list by client, Profile, Brief (the user's part), Timeline of calls.
   - Calls: chips, picker, suggestions (deterministic).
   - New project inline; Found for you (deterministic clusters; naming by a model run).
   - Glossary → vocabulary hint; `context` and the minutes guard read the project's recent minutes.
2. **The ledger (taska port).**
   - `project/ledger.rs` (events, fold, level rules, quote check, pins, undo), evidence from calls and calendar, the
     CLI, taska's tests ported.
   - The project-sync skill (extract, dependencies, verify), guarded, after minutes and nightly; backfill on request.
   - The Work tab, action item ↔ task links, done shown on both sides, To review.
3. **Brief and Tasks.** Staleness and Asks, the computed Brief, meeting prep on the Calendar, the Tasks activity,
   people priorities, Claude · <project>, ⌘K targets, verdict stats.
4. **More sources.** Teams chats as evidence, then channels (admin consent); mail (`Mail.Read`, a separate opt-in:
   threads to a list, replies to the project's follow-ups); local folders (the brain); pull requests.

Each phase ends with the usual checks (clippy, tests, the Windows check, the UI's node tests, `window --selftest`)
and a run on real calls before the next starts.

## Changes to other documents
- `plugins-design.md`: the task tracker is no longer the example plugin (it is core now); the terminal and a calendar
  action stay as examples. The "action items vs tasks" rules move here unchanged, except that the store is Echo's.
- `CLAUDE.md`: a Projects section once phase 1 lands.

## Open questions
None blocking phase 1. Answered on 2026-10-29: see *Decisions so far*. Left for later: a notification when something new
waits in To review (the badge only for now).
