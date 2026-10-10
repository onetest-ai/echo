# Claude Code plugin

**Preconditions for all:** Claude Code installed and signed in.

### TC-CLA-001 · Connect Claude Code from the menu
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Claude Code → **Connect Claude Code (install the plugin)**.
2. Start a new Claude Code session; type `/echo:` and look at the suggestions.

**Expected**
- A "Claude Code connected" notification.
- `claude plugin list` shows `echo@echo`.
- The session offers `/echo:process-call`, `/echo:live`, `/echo:record` and `/echo:doctor`.

---

### TC-CLA-002 · Doctor in a session
**Priority:** P1 · **Platform:** macOS

**Steps**
1. On a set-up machine, run `/echo:doctor`.
2. Turn off Echo's microphone permission in System Settings; run `/echo:doctor` again.

**Expected**
- Step 1: one line that Echo is fine; nothing is asked or downloaded.
- Step 2: Claude names the missing permission, opens its settings page and explains what to
  click; it never changes the setting itself.

---

### TC-CLA-003 · Notes are written after a call
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Keep a Claude Code session open.
2. Record a test call of at least 4 minutes with real back-and-forth (a decision, a task with
   an owner and a date).
3. Wait for the transcript.

**Expected**
- Without being asked, the session processes the call: `notes.md` and `actions.json` appear.
- A "Notes ready" notification. The Echo window's Notes and Actions tabs fill in.
- The notes have a summary, decisions and next steps with time chips; the task appears in
  Actions with its owner and due date.
- Claude offers follow-ups (recap email, tasks) and **sends nothing** until you say yes.

---

### TC-CLA-004 · Only one session takes a call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Open two Claude Code sessions in the notes folder.
2. Record and finish a call.

**Expected**
- Exactly one session writes the notes; the other does nothing for that call.

---

### TC-CLA-005 · Notes only in the chosen folder
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Claude Code → **Write call notes in** → pick a folder A.
2. Have one session open in folder A and one in an unrelated code project. Record a call.

**Expected**
- Only the session in folder A processes the call; the coding session is not interrupted.

---

### TC-CLA-006 · Missed calls are caught up
**Priority:** P2 · **Platform:** macOS

**Steps**
1. With no Claude Code session open, record a 4-minute call.
2. An hour later, open a session in the notes folder.

**Expected**
- The new session picks up the call and writes its notes (calls up to 48 hours old).

---

### TC-CLA-007 · Live pane
**Priority:** P2 · **Platform:** macOS

**Steps**
1. In a session, start recording a call. Run `/echo:live` if the pane doesn't open by itself.
2. Talk for 5 minutes.

**Expected**
- A band above the prompt shows the recording status with `[ Live ]`.
- The pane shows the transcript as it happens and, after a few minutes, a rolling summary,
  highlights and actions.

---

### TC-CLA-008 · Your own notes during the call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. During a recorded call, menu → **Add a note…** with a point you want kept ("ask legal about
   the contract").

**Expected**
- After the call, the final notes keep that point and use it as part of their structure.

---

### TC-CLA-009 · Write notes from the window (no session open)
**Priority:** P1 · **Platform:** macOS

**Preconditions:** Claude Code installed and signed in; no Claude Code session open.

**Steps**
1. Record a 4-minute test call with a decision and a task.
2. Wait for the "Transcript ready" notification.
3. Open the call → Notes → **Write notes**.

**Expected**
- The notification says the transcript is ready and to click Write notes.
- The tab shows "Writing notes…" with the model name, then the notes, within about two minutes.
- The Actions tab fills in at the same time. A "Notes ready" notification appears.
- **Show log** opens `claude.log`: the commands Claude ran, the time taken and the cost.

---

### TC-CLA-010 · Claude Code signed out
**Priority:** P1 · **Platform:** macOS

**Steps**
1. In a terminal: `claude` → `/logout`.
2. Open a transcribed call without notes → **Write notes**.
3. Sign in again (`claude` → `/login`), then click **Try again**.

**Expected**
- Within seconds: "Claude Code needs you to sign in", with what to do, a notification, and
  **Try again** / **Show log**.
- After signing in, Try again writes the notes.

---

### TC-CLA-011 · Notes model
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Claude Code → **Notes model…** → enter `haiku`; write notes for a call.
2. Enter a name that doesn't exist (`no-such-model`); write notes again.
3. Enter `default`.

**Expected**
- Step 1: the log's first lines show the haiku model.
- Step 2: the run fails with "Claude Code can't use the model…" and where to change it.
- Step 3: the run uses whatever Claude Code itself is set to (for example a gateway or local
  model configured in `~/.claude/settings.json` or the shell profile).

---

### TC-CLA-012 · The run can only touch the call
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Set Claude Code's own default permission mode to `auto` or `bypassPermissions`.
2. Write notes for a call; open **Show log**.

**Expected**
- The log shows only `echoctl` commands, reads in the data folder and writes in the call folder.
- Anything else (web, other shell commands, files elsewhere) appears as "Not allowed" and is
  not done, whatever the user's own default mode is.

---

### TC-CLA-013 · A session and Write notes at once
**Priority:** P2 · **Platform:** macOS

**Steps**
1. With a session open in the notes folder, finish a call; while that session writes the notes,
   click **Write notes** in the window.

**Expected**
- The window says a Claude Code session is writing the notes right now; only one set of notes is
  written.

---

### TC-CLA-014 · Record and stop from a chat
**Priority:** P2 · **Platform:** macOS

**Steps**
1. In a session with no call going, type "record my next call in Ukrainian".
2. Join a Teams call; after a minute, type "stop recording".

**Expected**
- Step 1: "Armed — recording starts when your next Teams call begins"; the octopus turns green.
- The recording starts with the call, pinned to Ukrainian; "stop recording" ends it and the
  transcript follows.
