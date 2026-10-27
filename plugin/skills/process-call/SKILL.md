---
name: process-call
description: Turn a recorded Echo call into a transcript and useful notes — transcribe on-device, then write notes from a template (summary, decisions, action items, open questions), structured action items, and vocabulary fixes for misheard names and terms. The Echo app runs it by itself after a call; use it when the user asks to process, transcribe, summarize or re-run a call.
---

# echo: process a call

Runs **unattended**: the Echo app starts it by itself 15 minutes after a call (or after the
user's last note), or when the user asks (Write minutes). Do every step without asking the user
anything; the only thing that waits for them is the follow-up offers at the end, and nothing is
ever sent without an explicit yes.

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
When the Echo app started this run it already claimed the call; run by hand, `echoctl claim <dir>`
first — it fails only while Echo or another session is writing them (then stop); a call that has
minutes already can be done again.

## 2. Read the context bundle
`echoctl context <dir>` prints JSON — use its numbers, don't recompute them:
`duration_min`, `transcript_chars`, `chars_by_speaker`, `dominant_language`, `colleagues` /
`external`, `user_notes` + `user_headings`, `heading_budget`, `summarize` + `skip_reason`,
`previous_notes`, `vocabulary`, `template`, `frames`, `chat`, `me_profile`.

`me_profile` (if set) is the user's own description — role, team, the people and terms they
deal with, how they like notes and follow-ups written. Follow it for tone, emphasis and naming.

- If `summarize` is false: don't write notes or the action items (step 5), and leave any frames
  alone. A call with frames is always summarized: the shared screens are content even when little
  was said. Tell the user in one line (`skip_reason`) and stop after step 6 (title).
  **Unless the user asked for them explicitly** (the app's prompt says so, or the user asked you in
  this session): then write the notes anyway — the gate only keeps short calls from getting notes
  automatically. Not when `skip_reason` says there is no transcript yet: step 1 first.

## 3. Template
`context` already picked it: `template.sections` is the list of `heading` + `instruction` to
follow (user templates in `<data folder>/.echo/templates/` override the built-in ones).

## 4. Write `<dir>/notes.md`
Name the speakers first (4a, if the transcript has "Speaker N" labels), then write:
- Start with `# {title}` and one line: date, duration, participants (colleagues, then external).
- **If the user took notes** (`user_notes`): their notes are the backbone. Keep every point
  they wrote, expand it from the transcript, and use their `user_headings` as section headings
  before the template's. Never drop or contradict something the user wrote.
- Otherwise use the template's sections, at most `heading_budget` headings in total (merge or
  drop the least relevant). Follow each section's instruction.
- Write in `dominant_language`. Keep names, products and figures exactly as said.
- Remote speakers share one track. It is labelled "Remote", or with people's names where Teams
  showed who was speaking, or "Speaker 1", "Speaker 2", … where Echo told voices apart without a
  name (one label per voice, across the whole call). Names Echo put there are reliable. Attribute a
  statement to a named person only when the transcript makes it unambiguous (they introduce
  themselves, or answer when addressed by name); a speaker you named in step 4a counts as that
  person everywhere. Otherwise say "someone on the call". "Me" is the user (`me.name` if set).
- If `previous_notes` exists, read it and add a short "Since last time" line when there is real
  continuity (a decision revisited, an action item followed up). Skip it otherwise.
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
  The live pane turns these into jumps into the transcript.

### Shared screens (`frames`)
If `frames` is non-empty, these are snapshots of what was on screen (slides, documents, demos),
each with `t` (seconds into the call, same clock as the transcript). Look at them with Read —
all of them if ≤ 12, otherwise the ones nearest to decisions, numbers and "as you can see"
moments plus an even spread. Use what they show to fill gaps the audio leaves (figures on a
slide, names on a diagram, the agenda) and add a short **On screen** section: one bullet per
meaningful frame, `[mm:ss]` + what it showed, only facts visible in the image: describe what
you can actually read or see in it, never what the conversation at that moment suggests it
should show; if you can't tell, write "shared screen" and nothing more. A frame's `shared_by`
says whose screen it was, not who presented it. Skip frames that are just video tiles or the
Teams UI.

Then prune: keep exactly the frames the **On screen** section cites and move the rest to Echo's trash (video
tiles, Teams UI, near-duplicates of a kept slide) — `echoctl frames <dir> --keep
<file1>,<file2>,…` (names as in `frames`), or `--keep none` if none was useful. Frames you
didn't look at (more than 12) are judged by their neighbours: keep one per distinct slide.

### Meeting chat (`chat`)
The meeting's Teams chat: `[{t, from, text, files}]`, `t` in seconds on the transcript's clock
(negative = before the recording). It is the third stream next to voice and screen: what was
written, not said. Use it — links and files shared, questions asked in writing, numbers pasted,
"+1"s that are really decisions — and add a short **In the chat** section (sender, what, `[mm:ss]`)
for anything not already covered by the spoken notes. Written commitments are action items too.
Senders are real names: they can also confirm who a remote speaker was when the transcript
shows the same point being made. Skip greetings and "joining in 2 min".

### 4a. Name the speakers (only if the transcript has "Speaker N" labels)
Echo already names voices from Teams' own speaking marker where it could; the rest are
"Speaker N". `echoctl speakers <dir>` lists them: `id`, `label` (what the transcript shows),
speaking time, first lines. Never rename a speaker whose label is already a person's name. Give a "Speaker N" a name when
the transcript is clear about it — they introduce themselves ("this is Lena"), or someone
addresses them by name and that speaker answers — and the name is one of the attendees
(`colleagues` / `external`) or said in full on the call. Then run
`echoctl speakers <dir> 2="Lena Varga" 3="Kim Ito"` (it rewrites the transcript's labels).
Leave the others as "Speaker N"; never guess from who talked most or from the invite order. Do
this before writing the notes, and use the names in them.

## 5. Write the action items: `<dir>/actions.new.json`, then `echoctl actions <dir> --apply`
The action items from the notes, as a JSON array, written to `<dir>/actions.new.json` (never to
`actions.json` itself):
`[{"id": "<kept from the current list, or none>", "task": "...", "owner": "Me" | "<name>" | null,
   "due": "<as stated>" | null, "confidence": "high" | "low", "quote": "<short supporting quote>",
   "t": <seconds into the call where it was said>, "status": "todo"}]`
then run `echoctl actions <dir> --apply <dir>/actions.new.json`. It replaces `actions.json`, keeps
the status the user gave each item meanwhile (they tick items off in the window and the pane, also
while you work), gives new items an id and removes `actions.new.json`; if it says the list is
malformed, fix the file and run it again. On a re-run (`actions.json` exists: `echoctl actions
<dir>` prints it), keep each item's `id` when your item is the same action — even reworded — and
give new actions none.
`owner` is whoever committed. A spoken "I will…" belongs to the speaker who said it: `"Me"` only
when the user's own track ("Me") said it. A commitment written in the chat belongs to its sender:
`"Me"` when that is the user (`me.name`). `null` when nobody took it.
`high` only when someone clearly committed ("I'll send it by Friday"). Empty array if none.

## 6. Title (unscheduled calls only)
If the call has no meeting subject (folder name is just a timestamp or a generic title), write a
specific one-line title (≤ 60 chars, no trailing punctuation) and run
`echoctl title <dir> "<title>"` — it prints the call's folder afterwards (renamed to match at once
in a session; in a run the Echo app started, when the run ends): use that path.

## 7. Vocabulary (automatic — never ask)
Echo spells names and jargon better when they are in the call's vocabulary. A term you learn
here is used again only in calls with this meeting's subject or mostly these people, or whose
invite names it (`context.vocabulary` is this call's list). Learn from
this call: names of people, products, tools, companies, projects and acronyms that the
transcript **misspelled or garbled**, when you are confident of the right spelling (it appears
in `meeting`/invite, it is a well-known product clearly meant from context — e.g. "as no is used
internally for operations" → ServiceNow — or the speakers spell it out). Also add correctly
transcribed but rare names that will recur (e.g. a project codename).
- Add them yourself: `echoctl vocab add --call <dir> "ServiceNow" "Miro" …` (it validates: 2–50
  chars, ≤ 6 words, deduped, capped at 100 with the oldest dropped). Never `--always`: that list
  is the user's.
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
1. post the notes to the meeting's Teams chat;
2. create To Do tasks for my action items (with due dates);
3. draft a follow-up email to the attendees (recap + action items + owners) — on yes, write the
   body to `<dir>/follow-up.md` and open it in the user's mail app with `echoctl compose --to
   <emails> [--cc …] --subject "…" --body-file <dir>/follow-up.md` (the user sends it there; mark
   external recipients — other email domains — in your message before opening);
4. schedule the follow-up meeting that was agreed.
Use msgraph for the others when available. Draft first and show the draft; send or create only after
the user says yes to that specific item ("1", "send it"). If msgraph isn't available, offer the
drafts as text to copy.
