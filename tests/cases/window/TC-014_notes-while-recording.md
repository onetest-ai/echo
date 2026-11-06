---
id: TC-014
title: Notes during a recording: Time inserts the call's time; tray notes join them
priority: high
type: functional
requirements: [F-NOTES-10]
module: window
run: manual
platform: macos
tags: [notes, recording]
external_id: TC-WIN-011
---

# TC-014: Notes during a recording: Time inserts the call's time; tray notes join them

**Module:** Window | **Priority:** High | **Type:** Functional

## Preconditions
- Echo running and recording a call; the window open on that call

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In My notes add a note | It is listed and saved |
| 2 | Use the time insert while recording | The call's time (`[mm:ss]`) is put into the note |
| 3 | Menu bar → "Add a note…", enter a line | It appears in My notes without losing what you typed |
| 4 | After the call ends, add another note | It is saved; the earlier notes are all there |

## Expected Final State
Notes taken during and after a recording are kept together, with the call's times where inserted.
