# Verification — an independent check of every claimed change

You did not extract these. For each packet decide whether the quote, read with its context, states
the **claimed change on this specific node** (its `path` shows workstream › story › task). Only
`supports` changes the tree.

1. `$L verify-prep --out <run folder>/packets.json` — its answer may carry `me_patterns`: how the
   user ("Me") works, from their About me, and `thread_scope` (see "One thread at a time"). Use it to read the context (e.g. "submitted" means the
   prep is done; work Me "passed to" someone is that person's), never instead of the words.
2. Read that file. For each packet write `{"mid": "…", "verdict": "supports|weak|wrong", "reason":
   "≤ 15 words"}` into `<run folder>/verdicts.json` (a JSON array covering every packet).
3. `$L verify-apply --file <run folder>/verdicts.json`

Verdicts
- `supports` — the words say this change happened to this node: `held` = the session took place;
  `done` = completed, received, working; `due` = this node's date was set or moved to roughly that
  date; `delayed` = it slipped without a new date; `blocked` = stuck; `progress` = partial progress
  already made; `cancelled` = someone decided not to do it; `reassign` = a new owner stated;
  criterion met = the context says it is now true; an answer = the words answer this question (as
  `claimed` puts it, near enough); a risk's response = the words say this is being done about this
  risk (a plan agreed, a fallback, someone owning it — a wish is not a response); a dependency = the
  waiting work can't finish or start until the named thing happens; waiting on a person = held up
  by that person's action; "covers it" (a `mention` with `covers`) = the words say this task's outcome
  happened, or that it isn't needed any more — not that it is still to do, planned or only talked about
  ("we still need to send X" is `wrong`).
- **Presupposition counts.** When the context treats an event as having happened ("how was the
  billing session?", findings reported from it), that supports `held` / `done`.
- **A recording is proof.** When the item is itself the session (its transcript is that meeting),
  that supports `held` for that session.
- **A promise is not progress.** "I'll send it" supports neither `progress` nor `done`.
- **One thread at a time.** A packet with `also_in` comes from an item shared with other threads; `thread_scope` in
  verify-prep's answer says what this thread is about (notes, brief, terms). `supports` only when the words are
  about this thread's work — the node's `path` is in it, and "A recording is proof" still holds for its sessions; a
  quote plainly about another thread's work is `wrong`, whatever the node's title.
- `weak` — the right session, person or topic, but the words don't state the change.
- `wrong` — contradicts the claim, or is about a different node (a quote about another story's
  session is wrong even when the topic is close).

Open nothing but the packets file.
