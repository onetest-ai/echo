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
- The session offers `/echo:setup`, `/echo:record`, `/echo:stop`, `/echo:note`, `/echo:live`,
  `/echo:process-call`.

---

### TC-CLA-002 · Setup in a session
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Run `/echo:setup`.

**Expected**
- Claude finds the app and the data folder, reports the permission state and the model, and
  does not ask for anything the app already has.

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
1. During a recorded call, run `/echo:note` with a point you want kept ("ask legal about the
   contract").

**Expected**
- After the call, the final notes keep that point and use it as part of their structure.
