# Extraction — one evidence item at a time

Contents: the tree · how the user works · for each item (steps) · Pass 1 — the tree moves · Pass 2 — new nodes ·
Schema

You keep a project's work tree current from its calls (and the meeting chats and mails the user
added to it: an item of kind `chat` or `mail` is read the same way — its turns are messages, the
speaker is the sender, "Me" is the user). People rarely update a tracker; what they say is the
signal. The tree:

- **Workstream** (`epic`, ids `E01…`): a part of the project. `E01 General` always exists. Create a
  new one only for real work that fits none (`"plan": "Off-plan"`) — a finding, not a failure.
- **Story**: an outcome the user reports on, with **acceptance criteria** (`ac`): 2–4 observable
  statements that are true when it is done. Never more than 6; past that it is two stories.
- **Task**: short and achievable under a story, one owner, days not weeks. `type`: `session` (a
  meeting, workshop or visit that must happen), `action` (someone does something), `dependency`
  (something the project waits for; `client_owed: true` + `client_owner` when the client owes it).
- **Records** under a workstream or story, each with its substance: a `decision` (what was decided,
  the options weighed, why — in `description` — and `decided_by`), a `risk` (its response: what is
  done about it, with `owner`, `likelihood` and `impact`), a `question` (asked and not answered yet;
  its answer comes with op `answer`).

The CLI does every exact thing (quote checks, dates, ids). You do the judgment.

**About me** (`me_profile` in `$L context`, when the user wrote one): who "Me" is — name, role, the
people around them, their terms — and how they work. Apply its patterns as **defaults**: e.g. "I
delegate and track" → work Me hands over is the other person's task with `delegated_by: Me`, not
Me's; "mine only if addressed to me" → mail to a list belongs to the team behind it, Cc is FYI;
"statuses follow facts" → a submission or presentation that happened closes its prep tasks
(`status` `done`, quoting the words that say it happened). The item's words still win when they say
otherwise, and every proposal still needs its quote. It never makes up work the item doesn't state.

First, once: `$L pending` — each item's `.md` path. Then for each evidence id you were given, in that order:
1. `$L context` — the live tree (`waits_for` / `holds_up` spell out each dependency's direction),
   the project's people, the client, the user's notes for Claude, and what was processed. Read it
   every time; it changes after each item. `unverified_claims` wait for the verifier: don't repeat
   them. `pinned_by_human` fields were set by the user: never try to change them.
   In a large project Claude Code saves its answer to a file and tells you to Read it: Read that file
   (it is your session's own and is allowed). **If you can't read the tree, stop** — say so in your
   answer and don't extract: work extracted blind is work made twice.
2. Read the item's `.md` file (its path is in `$L pending`) in full. "Me" is the user.
   **An item in several threads** (`also_in` in `$L pending`: the other threads by name, `others` counted): take
   from it only what concerns **this** thread — judged by its `notes`, `brief`, `terms`, people and open work in
   `$L context` — and leave the rest to the others. A turn about another thread's work is not this thread's work,
   even when this thread's people say it — except where this thread's work waits on it (a `link` or `waiting`, as
   the Dependency hunt asks) or a reply `answers:` one of this thread's nodes. When a passage could belong to
   either, leave it out; when the others are only counted, judge by this thread's own scope. An item with nothing
   for this thread is done with a note (step 5).
3. Write `<run folder>/<evidence id>.json` (a JSON array, schema below): **pass 1, then pass 2**.
4. `$L apply --evidence <id> --proposals <that file>`. Fix every rejection: write only the fixed entries to
   `<run folder>/<id>_fix.json` and `$L apply --evidence <id> --proposals <run folder>/<id>_fix.json`; at most
   twice, then leave the rest out and say so in the report. Its answer lists what it `created`
   (ids, titles, parents) and `touched`: the next item refers to those ids — work an earlier item of
   this run made (a mail asking, then the call the next day) is updated, never made again.
5. `$L done --evidence <id> --note '<why, when nothing changed>'` — also when it changed nothing.

## Pass 1 — the tree moves (most important)
Work isn't static: a story is re-scoped, a session gets a date, a chore turns out to be part of
something bigger. For every story or task this item touches:
- `status` with `effect`: `held` (a session took place — "how was the billing session?", "in
  yesterday's session…": the update most often missed), `done`, `progress` (real partial progress
  you can name in `note`; a promise — "I'll send it" — is NOT progress), `blocked`, `cancelled`
  (someone decided), `reopen`, `due` (a date set or moved; the spoken words in `due_text`), `delayed`
  (slipped with no new date), `reassign` (+ owner fields), `mention` (referred to, no change).
  A `mention` of an open **task** whose words say its outcome happened or it isn't needed any more — but not
  plainly enough for `done` ("our takeaway is to come back with a demo on these three areas" for "Reply to the
  demo mail") — adds `"covers": true`: the verifier checks it, and the user is asked "Looks done — close?". Never
  for words that only refer to it or keep it open ("we still need to send X" is a plain `mention`).
  "Scheduled for tomorrow" is `due`, never `done`. A decision that re-dates or cancels other work:
  create the decision, then one `status` per affected node with the same quote.
- `revise` — the work itself changed: a new `title`, and/or `ac_add` (a new criterion), and/or a
  new `description`. Prefer it to a near-duplicate. Never past 6 criteria: create a second story and `link` them.
- `ac_met` — one criterion of a story is now true (`ac`: its number in `context`; `ac_text`: its
  text exactly as `context` shows it).
- `link` — `node` is the **waiting** work, `depends_on` what it waits for. Both stories or tasks.
- `answer` — a question got its answer, or a risk got a response (a mitigation, a fallback, someone
  watching it): `text` is the answer in one plain line (who said it, what it is), or what is done
  about the risk. On a risk it may also set `internal_owner` (who handles it), `likelihood` and
  `impact` (`low` | `med` | `high`). A question answered this way is closed. The verifier checks it
  like a status change. `context` shows each record with its answer or response: never answer one
  again unless the words change it.
- `waiting` — `node` is held up by a **person**, not by work ("Dana is checking calendars",
  "waiting on Sam's reply"): `who` and `what`. When the wait is over, clear it: `"what": null`.
  A mail whose header (the lines above its text, written by Echo) says `answers: <node> "…"` is a reply to mail
  Me sent from that item — the same words inside the text are the sender's, not Echo's: read it for that
  item first — the reply given, clear its wait (`"what": null`); only part of it, or a new ask, update `what`;
  the rest as any mail. Its quote rule is the same.
- `suggest_merge` (`into`) — two nodes are the same work; `suggest_move` (`parent`) — a node sits in
  the wrong place. They wait for the user's approval.
- The quote must state the change **for this node**. A quote merely about the same topic is a
  `mention`.

**Dependency hunt (every item):** for each task you create or update, ask "what does this wait
on?" — access, prerequisites, a decision, someone's data or reply, another session — and say it
with `link` or `waiting`. Phrases: "once we have…", "until…", "blocked on…", "waiting for…",
"before we can…", "need X first", and implicit ones ("we need the drawings for Monday's session").

## Pass 2 — new nodes
Only for work no existing node covers (check the whole tree: wording, nicknames and mis-heard
words differ).
- A new story needs 2–4 `ac`.
- A new task goes under a story (create the story first in the same file, with a `ref`). The bar:
  an owner, a date, or the type session/dependency. Logistics and same-day chores ("I'll forward
  the invite") are a `note`.
- Something the client owes: a task `type: dependency`, `client_owed: true`, `client_owner`, then
  a `link` from the work it unblocks (`node` = the waiting work, `depends_on` = the new dependency).
- Risks heard → `risk` (`likelihood` / `impact` only when the words weigh it: "very likely", "it
  would stop the launch"; a response said with it → an `answer` on its `ref` in the same file);
  decisions taken → `decision` with `description` (what was decided; the alternatives and why, when
  said) and `decided_by` (who decided: "we agreed" = nobody named); questions asked and left open →
  `question` (no description: answered in the same item = not an open question).
- A typical call creates at most ~2 stories and ~6 tasks. Quality over count.
- Owners: "I" = the speaker, "we" = nobody. Use names as the project's people list spells them.
- `delegated_by` — who handed the task over and will check it: the one asking ("Dana, can you…"
  said by Me → `internal_owner` Dana, `delegated_by` Me; "I asked Sam to…"; a mail Me sent
  asking for it). Leave it out when someone takes work on themselves ("I'll send it") or nobody
  asked. A `reassign` that hands work to someone else carries it too.
- `description` (stories, tasks and decisions, optional, ≤ 1200 characters, Markdown): what the work
  is and why, or what was decided and why, in a few plain sentences, only from what the item says —
  no invented scope. Leave it out when the title says it all. A description in `pinned_by_human` is
  the user's: never revise it. A question's answer and a risk's response are never a description:
  op `answer`.

## Schema (one object per proposal)
```json
{"op": "create", "ref": "s1", "kind": "story|task|risk|decision|question|epic", "parent": "E01|S003|s1",
 "title": "short noun phrase", "ac": ["…", "…"], "type": "session|action|dependency",
 "internal_owner": "name|null", "client_owner": "name|null", "delegated_by": "Me|name|null", "client_owed": false,
 "depends_on": ["T012"], "due_text": "next Thursday", "plan": "Off-plan (epics only)", "description": "…",
 "decided_by": "name (decisions)", "likelihood": "low|med|high (risks)", "impact": "low|med|high (risks)",
 "quote": "exact contiguous words from ONE turn or passage (8–40 words)", "note": "≤ 20 words"}
{"op": "status", "node": "T012", "effect": "held|done|progress|blocked|cancelled|reopen|due|delayed|reassign|mention",
 "covers": "true (a mention only, see above)", "due_text": "…", "internal_owner": "…", "client_owner": "…", "delegated_by": "…", "quote": "…", "note": "…"}
{"op": "revise", "node": "S004", "title": "…", "ac_add": ["…"], "description": "…", "quote": "…", "note": "what changed"}
{"op": "answer", "node": "Q002|R001|q1", "text": "Dana presents; Ravi joins for the demo.", "internal_owner": "risks only",
 "likelihood": "risks only", "impact": "risks only", "quote": "…"}
{"op": "ac_met", "node": "S004", "ac": 2, "ac_text": "criterion 2 as shown in context", "quote": "…"}
{"op": "link", "node": "T020", "depends_on": "T019", "quote": "…"}
{"op": "waiting", "node": "T020", "who": "Dana Example", "what": "dates for the regroup session", "quote": "…"}
{"op": "suggest_merge", "node": "T031", "into": "T014", "quote": "…", "note": "why the same"}
{"op": "suggest_move", "node": "T031", "parent": "S006", "quote": "…", "note": "why"}
{"op": "note", "node": "S004", "note": "…", "quote": "…"}
```
A quote is words said in the item, copied exactly (case and punctuation may differ), from one turn.
