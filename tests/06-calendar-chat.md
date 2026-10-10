# Microsoft 365: calendar and meeting chat

The default sign-in uses the CodeMie app registration, so these cases need an Contoso account,
or your organization's own app (client) ID (TC-CAL-003).

### TC-CAL-001 · Connect Microsoft 365
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → Calendar → **Connect Microsoft 365…**.
2. Sign in in the browser that opens.

**Expected**
- The system browser opens the Microsoft sign-in page; after signing in the tab says "Signed in to Echo.
  You can close this tab."
- A "Calendar connected" notification names your account.
- The menu lists today's upcoming meetings; window → Calendar shows the week.
- `~/Library/Application Support/echo/graph.json` exists and is readable only by you
  (`ls -l` shows `-rw-------`).

---

### TC-CAL-002 · Sign out
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Calendar → **Sign out**.

**Expected**
- `graph.json` is removed; the menu offers Connect Microsoft 365 again; the Calendar view
  falls back to today's agenda without errors.

---

### TC-CAL-003 · Use another app (client) ID
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Menu → Calendar → **Change app (client) ID…**, enter your organization's ID.
2. Connect Microsoft 365 again. Then set it back by entering `default`.

**Expected**
- The new ID is used for sign-in (the consent page shows that app); a changed ID requires a
  new sign-in.
- `default` returns to CodeMie.

---

### TC-CAL-004 · Record a meeting when it starts
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Create a Teams meeting 5 minutes from now with one other attendee.
2. Window → Calendar → click it → **Record when it starts**.
3. Join the meeting on time.

**Expected**
- The meeting shows as "Will record" (dashed purple).
- A minute before the start the icon turns green (armed); when you join, it records.
- After the call, the meeting shows as "Recorded" (green) and opens the notes or transcript.
- Leaving and rejoining after it was recorded does not start a second recording by itself.

---

### TC-CAL-005 · Join and record
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Shortly before a meeting starts, click it → **Join and record** (window or menu → Upcoming).

**Expected**
- Teams opens the meeting; Echo is armed and records as soon as you are in.
- The call's title and attendees come from the calendar event.

---

### TC-CAL-006 · Meeting marked no notetakers
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Create a meeting with `no_notetakers` in its title or description.
2. Open it in the Calendar view.

**Expected**
- The meeting is labelled **No notetakers**, so you know the organizer opted out.

---

### TC-CAL-007 · Declined and cancelled meetings
**Priority:** P2 · **Platform:** macOS

**Expected**
- Declined meetings and cancelled meetings appear greyed out (cancelled ones struck through)
  and are not offered for recording.

---

### TC-CHAT-001 · Chat of a scheduled meeting
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Before a recorded meeting starts, post a message in its chat.
2. During the call, post a message with a link and one with a file.
3. Ten minutes after the call, post another message.

**Expected**
- The call's **Chat** tab has the messages from about 2 minutes before the start to 2 minutes
  after the end, with authors and times; files are listed by name.
- The message posted 10 minutes after the end is not included.
- Chat that came from someone else renders as plain text (no formatting or links that run code).

---

### TC-CHAT-002 · Chat of an ad-hoc call
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Start a direct Teams call (not a scheduled meeting) and record it.
2. Send a chat message in that call's chat during the call.

**Expected**
- Within a few minutes the message appears in the call's Chat tab.

---

### TC-CHAT-003 · Calendar not connected
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Signed out of Microsoft 365, record a call.

**Expected**
- Recording and transcription work as usual; there is simply no Chat tab and no calendar title.
