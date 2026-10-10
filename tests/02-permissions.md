# Permissions (macOS)

Permissions belong to **Echo.app** when the app records. Reset them for a case with
`tccutil reset All ai.onetest.echo`.

### TC-PERM-001 · Prompts at first launch name Echo
**Priority:** P0 · **Platform:** macOS

**Steps**
1. Fresh Mac, first launch, finish the folder and model dialogs.
2. Answer each permission prompt with **Allow**.

**Expected**
- Microphone and System Audio Recording prompts appear now, not in the middle of the first call.
- Every prompt names **Echo** (not Terminal, not echoctl).
- System Settings → Privacy & Security lists Echo under Microphone and under Screen & System
  Audio Recording (System Audio Recording Only).

---

### TC-PERM-002 · Health shows everything granted
**Priority:** P0 · **Platform:** macOS

**Steps**
1. After TC-PERM-001, open the menu.

**Expected**
- The submenu reads **Health ✓**; inside, each permission is marked as granted.

---

### TC-PERM-003 · System audio denied
**Priority:** P0 · **Platform:** macOS

**Steps**
1. Reset permissions; at the System Audio Recording prompt click **Don't Allow**.
2. Open the menu.
3. Record a short Teams call (TC-REC-001).

**Expected**
- The menu shows **Health ⚠** with system audio marked as missing.
- **Ask for permissions…** / **Re-check permissions** explain where to enable it.
- The call is not silently recorded with an empty remote track without a warning (the
  remote side is flagged, or Health stays ⚠ with a clear reason).

---

### TC-PERM-004 · Microphone denied
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Reset permissions; deny Microphone, allow System Audio Recording.
2. Record a short call.

**Expected**
- Health ⚠ shows the microphone as missing.
- The other people are still recorded and transcribed; your side is empty.

---

### TC-PERM-005 · Grant a permission later
**Priority:** P1 · **Platform:** macOS

**Steps**
1. After TC-PERM-003, enable Echo under System Audio Recording in System Settings.
2. Menu → Health → **Re-check permissions**.

**Expected**
- Health becomes ✓ without reinstalling. If macOS requires it, Echo says to restart the app.

---

### TC-PERM-006 · Accessibility for mute sync and speaker names
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Health → **Ask for permissions…**; enable Echo under Accessibility.
2. Record a call (TC-REC-001) and run TC-REC-009 (mute).

**Expected**
- Echo appears under Privacy & Security → Accessibility.
- Mute sync works (TC-REC-009). Without Accessibility, recording still works; only mute sync
  and speaker names are missing.

---

### TC-PERM-007 · Screen Recording for shared screens (opt-in)
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Settings → check **Save snapshots of shared screens** (off by default).
2. If asked, allow full Screen Recording for Echo.
3. Record a call where someone shares a slide deck and changes slides.

**Expected**
- Without full Screen Recording, Echo shows a "Screen Recording needed" notification.
- With it, the call has a Screens tab with a handful of frames, one per slide change, not one
  per 3 seconds.
