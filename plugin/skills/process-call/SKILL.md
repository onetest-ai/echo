---
name: process-call
argument-hint: "[call folder | latest]"
description: "Use when the user asks to process, transcribe, summarize or write the minutes of a recorded Echo call again, and when the Echo app runs it after a call. Transcribes on-device, then writes minutes from a template (summary, decisions, action items, open questions), structured action items and vocabulary fixes for misheard names."
---

# echo: process a call

The call: $ARGUMENTS (none given: `latest`). Do every step without asking the user anything; only
the follow-up offers at the end wait for them, and nothing is sent without an explicit yes.

**Started by the Echo app** (`ECHOCTL_HEADLESS` is set, nobody is watching): the app claimed the
call already, `echoctl` is on PATH, and only the Echo CLI's `context`, `actions`, `speakers`, `title`,
`vocab add`, `frames` and `transcribe` on this call, reading this call's folder and the previous
minutes `context` names, and writing only `notes.md` and step 5's new list in this call's folder
are allowed (not the user's notes, not `actions.json` directly: step 5; the app's prompt names the
file to write step 5's new list to — use that name instead of `actions.new.json`). Put words with
spaces or punctuation in single quotes; `$` and backticks can't be used. `echoctl title` keeps the
folder's path until the run ends: go on using it.
Run `echoctl …` as one plain command per Bash call (no variables, `cd`, pipes or redirects), write
files with Write / Edit, and skip step 8's offers.

**The user's notes** (`user_notes`, `my-notes.md`) are theirs: read them, never edit, rewrite or
delete them.

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`) — the installed Echo
app's CLI. Calls live in the app's data folder (`echoctl app` → `calls`), one folder per
call; `latest` works wherever a call is expected. All paths below are inside that call folder.

## 1. Transcript
The app transcribes every call itself when it stops. Only if `transcript.md` is missing and
the WAVs are still there, run `echoctl transcribe <dir>` (Bash `run_in_background: true`).
When the Echo app started this run it already claimed the call; run by hand, `echoctl claim <dir> --session "${CLAUDE_CODE_SESSION_ID:-process-call}"`
first — it fails only while Echo or another session is writing them (then stop); a call that has
minutes already can be done again.

## 2. Read the context bundle
`echoctl context <dir>` prints JSON — use its numbers, don't recompute them:
`duration_min`, `transcript_chars`, `chars_by_speaker`, `dominant_language`, `colleagues` /
`external`, `user_notes` + `user_headings`, `heading_budget`, `summarize` + `skip_reason`,
`previous_notes`, `vocabulary`, `template`, `frames`, `chat`, `me_profile`, `projects`, `live_notes`, `links`.

`me_profile` (if set) is the user's own description — role, team, the people and terms they
deal with, how they like notes and follow-ups written. Follow it for tone, emphasis and naming.

- If `summarize` is false: don't write notes or the action items (step 5), and leave any frames
  alone. A call with frames is always summarized: the shared screens are content even when little
  was said. Tell the user in one line (`skip_reason`), skip steps 3–5, do step 6 (title), then stop.
  **Unless the user asked for them explicitly** (the app's prompt says so, or the user asked you in
  this session): then write the notes anyway — the gate only keeps short calls from getting notes
  automatically. Not when `skip_reason` says there is no transcript yet: step 1 first.

## 3. Template
`context` already picked it (the user's templates, made in Settings → Templates, come before Echo's). Read
`template.text` as the reference for the minutes' shape and follow it:
- Its **header** (everything before the first `## `) is how the minutes start:
  - `{value}`: fill it in — `context.template_values` says what each known one stands for (`{date}`,
    `{participants}`, `{previous}`…). One with nothing to fill (no thread, no previous meeting) goes, with the
    separator or words next to it; a line left with nothing goes.
  - Any other `{…}` is an instruction in the template's words ("{one line on why we met}").
  - A line that says when it applies ("only when…") is left out otherwise.
  - Everything else (frontmatter, a title format) is kept as written.
- Each `## ` **section** is a heading of the minutes, in that order, and the words under it say what to write there —
  including formats it shows (a table's columns, bullet shapes, bold labels).
`template.header` and `template.sections` are the same, split.

## 3b. Meeting type (before you write)
`context.meeting_type` is what kind of meeting it was so far: the user's choice (`by: "you"` — leave it), else a guess
from who was invited. `context.meeting_types` lists every kind the user has — `id`, `name`, and `about`: how to tell
it. Settle it from what was said and who took part, by those `about`s, and run `echoctl type <dir> <id>` when it
differs from the guess. Never for a memo. Settle it before step 4: a template's `{type}` is this one.

## 4. Write `<dir>/notes.md`

### 4b. A memo first: find its threads (only when `memo` is true)
A memo is the user alone, thinking aloud — no speakers (skip 4a), no chat, no screens. Its threads come from what
was said, never from a picker:
1. Read the transcript against `open_threads` (each: `id`, name, client, brief, terms, people).
2. Split the memo into stretches by the thread each is about — a memo can cover several, one after another, and
   come back to one. A stretch belongs to a thread only when its words clearly concern it: its name, client, people,
   terms or the brief's subject. Unsure, or about none: leave it out (it goes under "Other").
3. Write `<dir>/memo-threads.new.json` — `[{"thread": "<id>", "from": "mm:ss", "to": "mm:ss"}]`, ids only from
   `open_threads` — and run `echoctl memo threads <dir> --file <dir>/memo-threads.new.json`. Use the `parts` it
   prints (it drops unknown or closed threads and ones the user took this memo out of, saying why).
4. Write the notes with the memo template (`template`): one section per kept part, in the memo's order.
Action items: `owner` "Me" unless the user named someone. Step 6 always applies: the memo's title is "Memo" until
you name it from what was said.

### 4a. First: name the speakers (only if the transcript has "Speaker N" labels)
Echo already names voices from Teams' own speaking marker where it could; the rest are
"Speaker N". `echoctl speakers <dir>` lists them: `id`, `label` (what the transcript shows),
speaking time, first lines. Never rename a speaker whose label is already a person's name. Give a "Speaker N" a name when
the transcript is clear about it — they introduce themselves ("this is Lena"), or someone
addresses them by name and that speaker answers — and the name is one of the attendees
(`colleagues` / `external`) or said in full on the call. Then run
`echoctl speakers <dir> 2="Lena Varga" 3="Kim Ito"` (it rewrites the transcript's labels).
Leave the others as "Speaker N"; never guess from who talked most or from the invite order. Do
this before writing the notes, and use the names in them.

Name the speakers first (4a, if the transcript has "Speaker N" labels), then write:
- Start with the template's header.
- **If the user took notes** (`user_notes`): their notes are the backbone. Keep every point
  they wrote, expand it from the transcript, and use their `user_headings` as section headings
  before the template's. Never drop or contradict something the user wrote.
  Notes tied to a moment: `- [12:05] …` is about that time of the call — expand it from what was said
  around then and put it in the section on that topic; a `  > "…"` line under a note quotes the words
  it is about. `? …` is the user's question (an open question unless the call answered it), `! …` a
  decision they noted, `- [ ] [12:22] …` their to-do (also written `- [12:22] [] …`; an action item for Me), `⚑` a moment they
  flagged — make sure what was said then is covered.
- **The live notes** (`live_notes`, if present): decisions, action items and open questions a model
  kept while the call ran (`t` = seconds into the call), and `last_now`, what was going on near the end.
  They are a valuable checklist, not a source of truth: make sure each one is covered — a decision,
  an owner and a task, a question still open or answered later in the call — but take its wording,
  owner and time from the transcript, and drop one the transcript doesn't bear out (a live note can
  mishear, or be overtaken minutes later). `gaps` were never read live; past `covered_until` there
  are none.
- Otherwise use the template's sections, at most `heading_budget` headings in total (merge or
  drop the least relevant). Follow each section's instruction.
- Write in `dominant_language`. Keep names, products and figures exactly as said.
- Remote speakers share one track. It is labelled "Remote", or with people's names where Teams
  showed who was speaking, or "Speaker 1", "Speaker 2", … where Echo told voices apart without a
  name (one label per voice, across the whole call). Names Echo put there are reliable. Attribute a
  statement to a named person only when the transcript makes it unambiguous (they introduce
  themselves, or answer when addressed by name); a speaker you named in step 4a counts as that
  person everywhere. Otherwise say "someone on the call". "Me" is the user (`me.name` if set).
- If `previous_notes` exists, read it: real continuity (a decision revisited, an action item followed up) fills the
  header's "Since last time" line, when the template has one. No continuity: leave that line out.
- `projects` lists the projects this call belongs to: each one's `brief`, `notes` (what the user
  wants you to know), `terms`, `people` and the `highlights` of its `recent_calls`. Use them to name
  things the way the project does and to see continuity across calls with other titles (a decision
  from an earlier call revisited here). They are context, not content: write only what this call
  said.
- **Links into Echo.** `links` gives `echo://` links for this call's `threads` and `clients`, the
  `previous` meeting of the series and the `people` Echo knows. Use them as plain Markdown links
  where the name appears anyway: people in the attendees line (`[Jane Doe](echo://person/…)`), the
  thread or client once in the header, the previous meeting in "Since last time". Only links from
  `links`, copied exactly — never make one up. Times stay `[mm:ss]`.
- Ground every line in the transcript. Leave a section out rather than pad it.
- **Diagrams:** when a process, flow or architecture was walked through (or the user asks for
  one), draw it as a ```` ```mermaid ```` block (`flowchart TD`; one short line per node, `<br>` for
  breaks, `{{…}}` for human decisions). The Echo window draws it. Only steps that were said or
  shown.
- **Cite the moment.** End each summary, decision, open-question and action bullet with the
  transcript time it rests on, as `[mm:ss]` (from the `**Speaker** [hh:mm:ss]` headers in
  `transcript.md`; drop the hour when it is 0). One anchor per bullet, never a range: the turn
  that supports exactly that point — for a decision, where it was agreed; for an action, where
  it was committed — and of equally good turns, the first.
  Echo's window turns these into jumps into the transcript.

### Shared screens (`frames`)
If `frames.count` is above 0, the call has snapshots of what was on screen (slides, documents,
demos), each with `t` (seconds into the call, same clock as the transcript). `frames.sample` holds
up to 12 of them (all of them when there are no more): the biggest changes on screen plus an
even spread. On a longer call, `frames.index` (`frames.jsonl` in the call's folder) lists them
all, one JSON line each — Read it. Look at the frames with Read: the sample, and on a longer call
also the ones nearest to decisions, numbers and "as you can see" moments. Use what they show to fill gaps the audio leaves (figures on a
slide, names on a diagram, the agenda) and add a short **On screen** section: one bullet per
meaningful frame, `[mm:ss]` + what it showed, only facts visible in the image: describe what
you can actually read or see in it, never what the conversation at that moment suggests it
should show; if you can't tell, write "shared screen" and nothing more. A frame's `shared_by`
says whose screen it was, not who presented it. Skip frames that are just video tiles or the
Teams UI.

Then prune: keep exactly the frames the **On screen** section cites and move the rest to Echo's trash (video
tiles, Teams UI, near-duplicates of a kept slide) — `echoctl frames <dir> --keep
<file1>,<file2>,…` (names as in `frames.index`), or `--keep none` if none was useful. Frames you
didn't look at (more than 12) are judged by their neighbours: keep one per distinct slide.

### Meeting chat (`chat`)
The meeting's Teams chat: `[{t, from, text, files}]`, `t` in seconds on the transcript's clock
(negative = before the recording). It is the third stream next to voice and screen: what was
written, not said. Use it — links and files shared, questions asked in writing, numbers pasted,
"+1"s that are really decisions — and add a short **In the chat** section (sender, what, `[mm:ss]`)
for anything not already covered by the spoken notes. Written commitments are action items too.
Senders are real names: they can also confirm who a remote speaker was when the transcript
shows the same point being made. Skip greetings and "joining in 2 min".

## 5. Write the action items, then `echoctl actions <dir> --apply <file>`
The action items from the notes, as a JSON array, written to the file the app's prompt names
(`actions.new-<run>.json`), else `<dir>/actions.new.json` — never to `actions.json` itself:
`[{"id": "<kept from the current list, or none>", "task": "...", "owner": "Me" | "<name>" | null,
   "due": "<as stated>" | null, "confidence": "high" | "low", "quote": "<short supporting quote>",
   "t": <seconds into the call where it was said>, "status": "todo"}]`
then run `echoctl actions <dir> --apply <that file>`. It replaces `actions.json`, keeps
the status the user gave each item meanwhile (they tick items off in Echo's window, also
while you work), gives new items an id and removes `actions.new.json`; if it says the list is
malformed, fix the file and run it again until it applies. On a re-run (`actions.json` exists: `echoctl actions
<dir>` prints it), keep each item's `id` when your item is the same action — even reworded — and
give new actions none.
`owner` is whoever committed. A spoken "I will…" belongs to the speaker who said it: `"Me"` only
when the user's own track ("Me") said it. A commitment written in the chat belongs to its sender:
`"Me"` when that is the user (`me.name`). `null` when nobody took it.
`high` only when someone clearly committed ("I'll send it by Friday"). Empty array if none.

## 6. Title (unscheduled calls and memos)
If the call has no meeting subject (folder name is just a timestamp or a generic title, or it is a memo), write a
specific one-line title (≤ 60 chars, no trailing punctuation) and run
`echoctl title <dir> "<title>"` — it prints the call's folder afterwards (renamed to match at once
in a session; in a run the Echo app started, when the run ends): use that path.

## 7. Vocabulary (automatic — never ask)
A term you learn here is used again only in calls with this meeting's subject or mostly these people, or whose
invite names it (`context.vocabulary` is this call's list). Learn from
this call: names of people, products, tools, companies, projects and acronyms that the
transcript **misspelled or garbled**, when you are confident of the right spelling (it appears
in `meeting`/invite, it is a well-known product clearly meant from context — e.g. "as no is used
internally for operations" → ServiceNow — or the speakers spell it out). Also add correctly
transcribed but rare names that will recur (e.g. a project codename).
- Add them yourself: `echoctl vocab add --call <dir> "ServiceNow" "Miro" …`. Never `--always`: that
  list is the user's.
- Don't add common words, guesses, anything only said once in passing, or a term that was only
  *written* by the transcript (it appears where it makes no sense: the recognizer put it there).
- Never edit `transcript.md`; corrections go only into `transcript.corrected.md` if the user
  later asks for one.

## 8. Report + follow-up offers
If notes were written, run `echoctl html <dir> --auto` (skip it when started by the Echo app — the
app does it itself): when the user turned the minutes page on, it writes the notes as one
self-contained HTML page next to them and prints its path; otherwise it says it is off.

A few lines: title, duration, the summary, action items (owner — task), where the files are (and
the minutes page, if one was written), and one line "Learned: X, Y (undo: `echoctl vocab remove X`)"
if step 7 added terms.

Then **always** offer concrete follow-ups, numbered, tailored to this call — for example:
1. post the notes to the meeting's Teams chat (`echoctl m365 teams send --chat <its chat id> --file <dir>/notes.md
   --ai <your model>`; the `echo:microsoft-365` skill);
2. draft a follow-up mail to the attendees — recap, action items, owners: write it to `<dir>/follow-up.md`, then
   `echoctl m365 mail draft --to <emails> --subject "…" --file <dir>/follow-up.md --ai <your model>` (an Outlook draft
   for the user to send; mark external recipients — other mail domains — in your message first);
3. put the action items into the call's thread as tasks (the `echo:manage` skill).
Draft first and show it; send or create only after the user says yes to that item ("1", "send it"). Not signed in to
Microsoft 365: `echoctl compose --to … --subject "…" --body-file <dir>/follow-up.md` opens it in their mail app.
