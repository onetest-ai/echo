# Privacy

These are release blockers: Echo's promise is opt-in recording, on-device transcription and
text only.

### TC-PRIV-001 · Nothing records without opting in
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. With Echo idle (not armed, nothing scheduled), join three Teams calls in a day.
2. Check the calls list and the data folder.

**Expected**
- No call was recorded. No new folders under `calls/`, no `.wav` files anywhere in the data folder.
- The icon stayed white (idle) during every call.

---

### TC-PRIV-002 · Only Teams is recorded
**Priority:** P0 · **Platform:** macOS

**Steps**
1. During a recorded call, play a video with speech in a browser (with sound, on the speakers
   or headphones).

**Expected**
- The video's speech does not appear as **Remote** lines. (With speakers, faint pickup by your
  own mic as **Me** is possible; with headphones nothing.)

---

### TC-PRIV-003 · No audio kept
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. After several recorded calls, search the data folder and the config folder for audio:
   `find ~/Echo "~/Library/Application Support/echo" -name '*.wav'` (adjust to your data folder).

**Expected**
- No `.wav` files once each call is transcribed (only while a call is recording or
  transcribing).

---

### TC-PRIV-004 · Transcription stays on the device
**Priority:** P0 · **Platform:** macOS

**Steps**
1. Turn Wi-Fi off. Record a call (on a local or already-running Teams call, or with a second
   device in the room speaking to the Mac's mic), and let it transcribe.

**Expected**
- The transcript is produced offline.
- (Optional, with Little Snitch or `nettop`) during transcription Echo makes no network
  connections.

---

### TC-PRIV-005 · What does go online
**Priority:** P1 · **Platform:** macOS

**Expected**
- Echo only contacts: GitHub (update checks and the model download from Hugging Face),
  Microsoft (sign-in and Graph, only when the calendar is connected).
- Notes go through Claude Code only when you connected it; the app itself never sends a
  transcript anywhere.

---

### TC-PRIV-006 · Sign-in tokens are private
**Priority:** P1 · **Platform:** macOS

**Steps**
1. `ls -l "~/Library/Application Support/echo/graph.json"` (after TC-CAL-001).

**Expected**
- `-rw-------` (owner only). Tokens never appear in `app.log`.

---

### TC-PRIV-007 · Follow-ups are never sent without a yes
**Priority:** P0 · **Platform:** macOS

**Steps**
1. After TC-CLA-003, read Claude's follow-up offers; reply with something unrelated, or nothing.

**Expected**
- No email, Teams message or task is created. Only an explicit "yes" to a specific item sends
  or creates it.
