# Recording

A "test call" below means a Teams call with a second person (or a second account on a phone),
both speaking, at least 1 minute unless stated otherwise.

### TC-REC-001 · Record the next call from the menu
**Priority:** P0 · **Platform:** macOS

**Steps**
1. Not in a call. Menu → **Record next call**.
2. Check the icon and the menu status.
3. Start a Teams test call within 15 minutes. Talk for 2 minutes, both sides.
4. Hang up.

**Expected**
- After step 1: the icon turns **green**; the status reads "Armed — recording starts with the
  next call"; the record item now reads **Cancel**.
- When the call connects: the icon turns **orange**; the menu offers **Stop recording**.
- About 10 seconds after hanging up the recording closes and the icon goes back to idle.
- The call shows up in the Echo window, first as transcribing, then with a transcript.

---

### TC-REC-002 · Record a call already in progress
**Priority:** P0 · **Platform:** macOS

**Steps**
1. Join a Teams test call without arming.
2. Menu → **Record this call**.

**Expected**
- The menu says **Record this call** (not "next") while you're in a call.
- Recording starts within a couple of seconds; the transcript covers only what was said after
  you clicked.

---

### TC-REC-003 · Cancel an arm
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Record next call → then **Cancel**.
2. Start a Teams call.

**Expected**
- The icon returns to idle after Cancel; the call is **not** recorded.

---

### TC-REC-004 · Arm expires after 15 minutes
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Record next call. Don't start a call for 16 minutes.
2. Then start a Teams call.

**Expected**
- After 15 minutes the icon returns to idle (the arm expired).
- The later call is not recorded.

---

### TC-REC-005 · Stop recording by hand
**Priority:** P0 · **Platform:** macOS

**Steps**
1. During a recorded call, menu → **Stop recording**.
2. Stay in the call for another minute.

**Expected**
- Recording stops immediately; the call stays connected in Teams.
- The transcript ends at the moment you stopped.
- The rest of the call is not recorded, and Echo does not re-arm by itself.

---

### TC-REC-006 · Short drop and rejoin
**Priority:** P1 · **Platform:** macOS

**Steps**
1. During a recorded call, leave the call and rejoin within 5 seconds.

**Expected**
- One recording, not two. The gap is silence in the transcript timeline.

---

### TC-REC-007 · Switch audio device mid-call
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Start a recorded call on the built-in speakers and mic.
2. After a minute, connect AirPods (or another headset) and let Teams switch to them.
3. After a minute, disconnect them again.

**Expected**
- Recording continues across both switches; both sides are transcribed before, during and after.
- The transcript timeline stays in sync (no drift between your lines and theirs).

---

### TC-REC-008 · Speakers instead of headphones (echo)
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Record a call on the laptop speakers (no headphones). Let the other person talk a lot.

**Expected**
- The other person's words are not duplicated as **Me** lines in the transcript.

---

### TC-REC-009 · Mute in Teams
**Priority:** P1 · **Platform:** macOS · needs Accessibility

**Steps**
1. During a recorded call, mute yourself with Teams' microphone button.
2. Say a sentence you can recognize ("this should not be recorded").
3. Unmute and keep talking.

**Expected**
- The muted sentence does **not** appear in the transcript.
- Speech after unmuting is transcribed.

---

### TC-REC-010 · Add a note during the call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. During a recorded call, menu → **Add a note…**, type a note, confirm.

**Expected**
- The note is saved with the call (`my-notes.md` in the call folder) and shows up in the notes
  Claude writes later (TC-CLA-003).

---

### TC-REC-011 · Mac goes to sleep during a recording
**Priority:** P2 · **Platform:** macOS

**Steps**
1. During a recorded call, close the lid for 1 minute, then open it.

**Expected**
- The recording is closed at the sleep point and transcribed; nothing is recorded after wake
  unless you arm again.

---

### TC-REC-012 · Echo quits or crashes while recording
**Priority:** P2 · **Platform:** macOS

**Steps**
1. During a recorded call, force-quit Echo (Activity Monitor → echoctl → Force Quit).
2. Open Echo again.

**Expected**
- On restart, the unfinished recording is repaired and transcribed up to the crash point.
- If transcription fails, the call shows the error instead of retrying forever.

---

### TC-REC-014 · Ask before calls (prompt mode)
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Settings → Calls not on the calendar → **Ask when they start**.
2. Start a Teams call; answer the "Record?" dialog with no. Start another and answer yes.
3. Set it back to **Only when I click Record**.

**Expected**
- Echo's own dialog appears when each call starts (no Claude Code needed).
- "No" records nothing; "yes" records that call only.
- Calendar meetings follow their own setting (Calendar meetings → Ask before they start).

---

### TC-REC-013 · Very short call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Arm, then make a 30-second call.

**Expected**
- A transcript is made. No notes are generated for it (too short), and nothing errors.
