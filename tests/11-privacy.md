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
  Microsoft (sign-in and Graph, only when the calendar is connected), and a transcription server
  only when one is connected.
- A call's content leaves the Mac only through Claude Code (or the local model an AI profile
  names), with the call's AI profile: Echo runs it for the minutes 15 minutes after a recorded call
  when Claude Code is installed (menu → Claude Code → Write minutes automatically, on by default —
  off: nothing is sent until you click Write minutes), and for live notes during a call only if
  they are turned on. The minutes run reads the transcript, the user's notes, the invite (subject,
  time, organizer, attendees with their email addresses, the join link), the meeting chat, saved
  shared screens, the user's name and email from the settings, `me.md`, the vocabulary, the notes
  template, the minutes of the last earlier call with the same subject and, on a re-run, the
  current action items. Its permissions also let it read the rest of the data folder and the
  folder Echo runs Claude Code in, and run only the `echoctl` commands the minutes need (context,
  actions, speakers, title, vocab add, frames, transcribe: not settings, the calendar or chats);
  with the built-in profile, the user's
  own Claude Code settings apply too, with the tools they allow. With a transcription server
  connected, each utterance's audio also goes to that server (it keeps nothing). Apart from these,
  Echo sends none of it anywhere.

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
