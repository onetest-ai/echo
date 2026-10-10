# The Echo window

### TC-WIN-001 · Open and close the window
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Menu → **Open Echo…**.
2. Close the window with the red button.
3. Open it again from a recent call in the menu.

**Expected**
- The window opens; a Dock icon (the onetest logo) appears while it is open.
- Closing it removes the Dock icon; Echo keeps running in the menu bar.
- Opening from a recent call selects that call.

---

### TC-WIN-002 · Calls list and search
**Priority:** P1 · **Platform:** macOS

**Steps**
1. With a few recorded calls, open Calls.
2. Type part of a title, then part of an attendee's name in the search box.

**Expected**
- Calls are grouped by day, newest first, each with time, duration, people and a status tag.
- Search filters by title and by people.

---

### TC-WIN-003 · Call tabs
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Open a call that has notes, chat and actions. Visit Notes, Transcript, Chat, Actions.
2. In Notes, click a time chip such as `11:40`.
3. In Transcript, use "Find in transcript".

**Expected**
- Each tab shows its content; the counts next to Chat and Actions match.
- The time chip jumps to that moment in the transcript.
- Find highlights the matches.

---

### TC-WIN-004 · Tick off an action item
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Actions tab → tick an item. Close and reopen the window.

**Expected**
- The item stays ticked (crossed out) after reopening.

---

### TC-WIN-005 · Call without notes
**Priority:** P1 · **Platform:** macOS

**Steps**
1. Open a call that was transcribed but has no notes (no Claude Code connected).

**Expected**
- The Notes tab explains that notes come from Claude Code and shows the
  `/echo:process-call` command to copy.

---

### TC-WIN-006 · Delete a call
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Select a call → **Delete** → confirm.

**Expected**
- The call disappears from the list and from the menu's recent calls.

---

### TC-WIN-007 · The window updates live
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Keep the window open on the Calls list while a recording ends and transcribes.
2. Meanwhile, scroll a transcript and select some text in another call.

**Expected**
- The new call appears and its status changes on its own within a few seconds.
- The refresh does not reset your scroll position, selection or open tab.

---

### TC-WIN-008 · Calendar Day and Week
**Priority:** P1 · **Platform:** macOS · needs TC-CAL-001

**Steps**
1. Window → Calendar. Switch between **Day** and **Week**; use ‹ › and **Today**.
2. Resize the window from small to full screen.

**Expected**
- Day shows today; Week shows yesterday, today and the next 5 days.
- A red line marks the current time.
- The hour grid fills the window height (no large empty band at the bottom); at very small
  heights it scrolls instead.
- Overlapping meetings sit side by side, not on top of each other.

---

### TC-WIN-009 · Meeting card placement
**Priority:** P2 · **Platform:** macOS

**Steps**
1. In Week view, click a meeting in the leftmost column, then one in the rightmost column.
2. With a card open, scroll the grid and resize the window.

**Expected**
- The card opens next to the meeting you clicked: to its right when there's room, otherwise to
  its left, level with the meeting, always fully inside the window.
- It follows the meeting when you scroll or resize. **Close** dismisses it.

---

### TC-WIN-010 · Dark mode
**Priority:** P2 · **Platform:** macOS

**Steps**
1. Switch macOS to Dark appearance with the window open; then back to Light.

**Expected**
- The window follows the system appearance; all text stays readable in both.
