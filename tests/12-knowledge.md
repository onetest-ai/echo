# Knowledge vaults

Connecting a folder of notes, starting a vault, minutes saved into a vault, editing and asking. Use a **copy** of a
real vault (`rsync -a --exclude .git/index.lock`) or a made-up one, never the only copy of someone's notes, and a
made-up Teams call. Design: `.ai-run/guides/design/knowledge.md`.

### TC-KN-001 · Connecting changes nothing in the vault
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. Copy a real vault that is an Obsidian vault and a git repository, with some uncommitted changes.
2. In it, run `git status --porcelain > /tmp/before.txt` and note the newest file's time.
3. Knowledge → switcher → *Connect a folder…* → choose the copy → *Next* → *Next* → *Connect*.
4. Open a few pages, the Graph and the vault's Settings; then run `git status --porcelain > /tmp/after.txt`.

**Expected**
- *What Echo found* names the pages folder, the index, the notes folder with its count and naming, the skills, and
  warns that files are changed and not committed.
- `diff /tmp/before.txt /tmp/after.txt` is empty and no file in the vault is newer than before.
- Pages opens on the index; the tree hides the sources folder, templates and dot-folders.

---

### TC-KN-002 · A new vault, with and without git
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. Knowledge → switcher → *Start a new vault…*, name it "Test wiki", keep *Make it a git repository* on → *Create*.
2. Repeat on a machine (or a PATH) without git, name it "No git wiki".

**Expected**
- `<data folder>/knowledge/test-wiki/` holds the starter's 23 files; `git log` shows one commit; `git status` is
  clean. The vault opens on Home with its three steps (Add a source, Record a call, Ask your twin); its index page
  has no welcome card (spec-2 K11-09).
- Without git: the files are there and the message says the first commit wasn't made; nothing else fails.

---

### TC-KN-003 · Automatic minutes land in each vault
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. Have two vaults set to *Save each call's minutes here: Automatically*: a connected one (commits off) and the new one
   from TC-KN-002 (commits on), both on the default AI profile.
2. Record a made-up Teams call of a few minutes; wait for the minutes.

**Expected**
- The call page shows *Minutes → <both vaults>* while recording; the countdown names them.
- The notification says the minutes are saved to both. The chip reads *In … ↗* and opens the note.
- Each vault has the note in its notes folder and a row at the end of its notes table.
- Connected vault: nothing committed; `git status` shows only the note and the index as changed besides the user's
  own. New vault: one new commit with exactly those two files.

---

### TC-KN-004 · The user's edit to a saved note wins
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. After TC-KN-003, add a line to the connected vault's note in Obsidian (or any editor).
2. In Echo, the call's ⋯ → *Write minutes again*; wait.

**Expected**
- The note still has the added line. The chip says *… · not updated*; its offers (*Save as a new note*, *Replace
  with the new minutes*) each do what they say, and *Replace* asks first.

---

### TC-KN-005 · Calls on another AI profile aren't saved by themselves
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Record a call on a client's AI profile while the vault reads on the default profile.
2. After the minutes, the call's ⋯ → *Save to Knowledge…*.

**Expected**
- Nothing was saved automatically; the chip offers saving.
- The sheet warns, naming both profiles; saving works only through *Save anyway*.

---

### TC-KN-006 · A missing vault folder
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Rename a vault's folder (or unplug its drive), then let a call's minutes be written.
2. Put the folder back; on the chip or in the Inbox press *Save again*.

**Expected**
- The minutes are fine in Echo. The chip says *Couldn't save to …* and the Inbox has the item with the reason.
- After restoring, *Save again* saves the note and the Inbox item goes away. Meanwhile the vault's page said its
  folder isn't there and Settings → Knowledge marked it *folder not found*, without errors.

---

### TC-KN-007 · Two editors on one page
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Open a page in Echo's editor and change a line; do not save.
2. Change and save the same page in Obsidian.
3. Back in Echo, try *Save*; use *Compare*, then *Keep mine* (and, another time, *Take theirs*).

**Expected**
- Within a few seconds Echo says the page changed on disk and disables *Save*.
- *Compare* marks the lines only in your text and only on disk. *Keep mine* asks first, then saves your text;
  *Take theirs* drops yours and shows the file.

---

### TC-KN-008 · An Ask session continues in a terminal
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. In a vault read on *Claude Code (as configured)*, *Ask* → `/twin what pages mention Northwind?` (made-up
   content); let it answer; close the panel. (On a managed profile Echo offers no terminal line: check it isn't shown.)
2. In a terminal in the vault's folder, run `claude --resume` and pick the session.

**Expected**
- The answer names the pages it used. The same session continues in the terminal with its history.

---

### TC-KN-009 · `/twin` from any Claude Code session
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Vault Settings → Skills → *Use /twin from any Claude Code session* → *Install*.
2. In another project, start Claude Code and run `/<name> what do I know about Northwind?`.
3. Back in Echo, *Remove*.

**Expected**
- `~/.claude/skills/<name>/SKILL.md` exists, marked as Echo's copy; the skill answers from the vault.
- *Remove* deletes only `~/.claude/skills/<name>/`; nothing else in `~/.claude/skills` changes.

---

### TC-KN-010 · The graph moves under your hand
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Open a vault of a few hundred pages (a copy) → *Graph*. Wait until it stops moving.
2. Drag a page with several links a long way; let go. Drag the background; scroll to zoom; use *+*, *−* and *fit*.
3. Hover a page; click it; double-click another.
4. Type a word in *Find*, then press Enter.
5. Click a folder in the legend twice. Turn on *Its local graph*, then *Tags as nodes*.
6. Zoom in on a corner, then add a page linking to a page there in Obsidian (or any editor) and save it.

**Expected**
- It settles within a few seconds and then uses no CPU. The dragged page follows the pointer and its linked pages swing
  after it; let go, it settles again. Pan, zoom and the buttons do what they say.
- Hovering lights the page, its neighbours and their links and dims the rest; a click opens the card, a double click
  opens the page. *Find* rings the matches with their count; Enter flies to the most linked one and selects it.
- Turning a folder off removes its pages and adds a "1 folder hidden" pill. The local graph and tags move into place
  rather than being redrawn from scratch.
- Within a few seconds the new page appears beside the page it links to; the camera stays where it was. Labels never
  overlap; the light and dark themes both read well.

---

### TC-KN-011 · Ask: three modes, the cards, stop and resume
**Priority:** P0 · **Platform:** macOS, Windows

Use a **copy** of a vault (or a made-up one such as a new vault from the starter), never the only copy of someone's
notes. This starts the real Claude Code on the vault's profile.

**Steps**
1. Knowledge → the copy → *Ask* → mode *Ask*: "What do my pages say about Northwind?" (made-up content). Watch it stream.
2. Switch to *Curate*: "Add a line about the gateway question to the Northwind page." When the card appears, read its
   diff, press *Deny* once and ask again; this time press *Allow*.
3. Switch to *Research*: "Read https://example.com and tell me what it says." Answer the card that names the domain.
4. Ask something long and press *Stop* while it writes.
5. In a terminal in the vault's folder run `claude --resume` and pick the conversation.

**Expected**
- The answer streams without jumping; the view follows the bottom only while you are there. Tool rows say what was
  read or searched; the rail lists the pages used; cost and tokens show at the end.
- In *Curate* nothing is written before *Allow*; after a *Deny* the file is as it was. A vault that commits gets one
  commit `Ask: …` with just the edited file; one that doesn't says nothing was committed. The change is in Home's
  *What's new* as "Ask (Curate)".
- The *Research* card names the domain; *Ask* and *Curate* offer no web.
- *Stop* ends the turn, and the conversation can be continued by sending another message.
- `claude --resume` continues the same conversation with its history.

---

### TC-KN-012 · Ingest: nothing reaches the vault before you approve
**Priority:** P0 · **Platform:** macOS, Windows

Use a copy of a vault with git and note `git status --porcelain`. Sources are made-up: a meeting-notes file, a PDF
you made of a made-up page, and a made-up mail thread in your own mailbox (Microsoft 365 signed in).

**Steps**
1. Knowledge → *Ingest*. If it asks about the files already in the inbox, choose *Mark them done*.
2. *Add a source* → *Files*: the meeting file and the PDF; → *Mail or Teams*: the thread. Three sources are *New*.
3. Tick all three → *Prepare changes*. While the run goes, run `git status --porcelain` in the vault.
4. In the review, check each change's *came from* quotes. Reject the new page that came from one source; approve the
   rest → *Write N approved*.

**Expected**
- `git status` is unchanged while the run goes and until *Write*: the run works on a copy.
- The PDF's cards say "Echo can't check quotes in this kind of file" and show no quote; every quote shown is in its
  source.
- The rejected page's index row and log line are left out and the cards say so. After *Write* only the approved files
  changed (plus yours); the sources are *Done*; no source file was edited.

---

### TC-KN-013 · A change made in the vault while a run waits
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Start a run on a copy as in TC-KN-012 and stop at the review.
2. In Obsidian (or any editor) change a page the run also changed, and save.
3. Back in Echo, *Approve* that change and *Write*.

**Expected**
- The change says it "changed in the vault since the run" and shows what would be overwritten (*Compare*).
- Nothing is overwritten until you choose *Take the run's*; *Keep the vault's* leaves your text. The other approved
  changes are written.

---

### TC-KN-014 · Health: fixes are diffs, AI checks quote real lines
**Priority:** P1 · **Platform:** macOS, Windows

Use a copy of a vault with a few broken links and pages missing from the index (the starter plus edits will do).

**Steps**
1. *Health* → pick a broken link → read its diff → *Fix*.
2. Pick *Not in the index* → *Fix all* → untick one → apply.
3. *Run AI checks* (the vault's profile reads the pages); open a finding.
4. Look at Home's Health card.

**Expected**
- Every fix shows its diff before anything is written; a name guessed by edit distance is offered on its own finding
  and never by *Fix all*. A file you had changed is written but left out of the commit, and the sheet says so.
- The score rises and Home's card follows. Before the first AI check the card says "code checks only"; after it,
  Freshness counts.
- Each AI finding shows quotes that are on their pages (open the page and look); findings whose quotes aren't were
  dropped and counted ("N dropped: their quotes aren't in the pages").

---

### TC-KN-015 · Home of a vault with history and of a new vault
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Open the Home of a copy of a git vault. Save minutes into it, approve an ingest change and make a commit of your
   own in a terminal; edit a page in Obsidian.
2. Open the Home of a vault just made from the starter.

**Expected**
- *What's new* lists each change once, with the right "who": minutes, Ingest, Curate, Health, "You in Echo", your
  commit by its author, the Obsidian edit as *Changed outside Echo*; Echo's own commits are not taken for yours.
- Suggestions come only from real items (a meeting today with a client page, the newest note, a health finding) and
  fill the box without sending it.
- The new vault shows three steps (*Add a source*, *Record a call*, *Ask your twin*) and no suggestions or changes.

---

### TC-KN-016 · Web pages, http, and Microsoft 365 signed out
**Priority:** P1 · **Platform:** macOS, Windows

**Steps**
1. Ingest → *Add a source* → *A web page* → an https address of a page you control or a public documentation page.
2. Try an `http://` address, and an address of a page that is a PDF or a very short page.
3. Sign out of Microsoft 365 (Settings → Calendar & chat), then *Add a source* → *Mail or Teams*.

**Expected**
- The page is saved under `web/` in the sources folder as Markdown with a title and a source line; a *New* source.
- The `http://` address is refused before anything is requested, saying why; the PDF or empty page is refused with
  what to do ("save it as a PDF and add the file").
- The mail option says Echo isn't signed in and where to sign in; nothing is requested.
