# Transcription

### TC-TRN-001 · Transcript content and speakers
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. Record a 3-minute test call; take turns, each side says a few known sentences
   (for example, read a short news paragraph).
2. Open the call in the Echo window → Transcript.

**Expected**
- Your lines are labelled **Me**; the other side is named (with Accessibility) or **Remote**.
- The known sentences are recognisable, with names and numbers mostly right.
- Lines are in time order; a quick reply comes after the line it answers.
- No invented filler such as "Thanks for watching", "Subtitles by…" or "[music]".

---

### TC-TRN-002 · Audio is deleted after transcription
**Priority:** P0 · **Platform:** macOS, Windows

**Steps**
1. During a recording, open the call folder (window → Show folder): note `mic.wav` and `remote.wav`.
2. Wait until the transcript is ready; look again.

**Expected**
- After transcription the folder has `transcript.md`, `segments.json`, `meta.json`, but **no
  `.wav` files**.

---

### TC-TRN-003 · Second language
**Priority:** P1 · **Platform:** macOS

**Preconditions:** Menu → Settings → **Languages: …** includes English and Ukrainian (or another pair you speak).

**Steps**
1. Record a call that switches between the two languages, a few sentences each.

**Expected**
- Each sentence is in the language it was spoken in. Nothing is translated into the other
  language.

---

### TC-TRN-004 · Languages restricted
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Set menu → Settings → Languages to English only.
2. Record a call in English with a strong accent.

**Expected**
- The transcript is in English throughout; no line comes out in Russian, Ukrainian or another
  language.

---

### TC-TRN-005 · Pin a language for one call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Record ▸ **Only Ukrainian** (or another listed language) before a call.
2. Record a call in that language.

**Expected**
- The transcript is in that language only; the workspace language setting is unchanged
  for later calls.

---

### TC-TRN-006 · Silence and noise
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Record a call where nobody speaks for 2 minutes in the middle (keyboard typing allowed).

**Expected**
- No made-up text during the silence.

---

### TC-TRN-007 · Long call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Record a call of 60 minutes or more.

**Expected**
- The transcript finishes (time it: roughly a few minutes per hour on Apple Silicon).
- The Mac stays usable during transcription; the transcript then appears, with a "Transcript
  ready" notification when Claude Code is set up.
