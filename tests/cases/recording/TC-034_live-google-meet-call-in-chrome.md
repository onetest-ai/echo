---
id: TC-034
title: A live Google Meet call in Chrome is recorded from Chrome's real audio and the mic Chrome uses; another tab's sound is in the remote track
priority: critical
type: functional
requirements: [F-REC-21, F-REC-07]
module: recording
run: person
platform: macos
tags: [recording, google-meet, chrome, live-check]
---

# TC-034: A live Google Meet call in Chrome

**Module:** Recording | **Priority:** Critical | **Type:** Functional

## Preconditions
- Echo (this build) running as the user's app, Microphone and System Audio Recording granted to it, Accessibility granted (to read Chrome's window title); Google Chrome with a Google account that can start a Meet
- A second device (a phone) to join the Meet as the other side; headphones on this Mac
- Settings → Recording: "Ask" for calls nobody scheduled

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | In Chrome open `https://meet.google.com/new`, join; join from the phone too | Within a few seconds Echo's dialog "A Google Meet call started. Record and transcribe it? Everything Chrome plays is recorded, from every tab." with "Not now" / "Record" |
| 2 | Click "Record"; talk from both sides for about a minute | The pill shows a recording; `echoctl status` has `watcher.app` "meet" |
| 3 | Play a YouTube video in another Chrome tab for 10 s; then stop it | (Expected by design: that sound is in the remote track — the dialog said so) |
| 4 | Switch Chrome's front tab to another site for 10 s, then back | The recording goes on (a tab switch is no hang-up) |
| 5 | Leave the Meet from Chrome; wait 15 s | The recording stops (`stop_reason` "call_ended"); the call's transcript has both sides' words, Me from the mic, the phone's words as the remote speaker |
| 6 | Open Chrome alone on a site that uses the mic (a voice-search, a dictation page) with no Meet tab and no Meet meeting in the calendar | No "Record?" dialog; nothing records |

## Expected Final State
One call recorded from the Meet; nothing recorded for Chrome's other use of the mic.

## Notes
- The file-fed cases (`TC-031`–`TC-033`) check the watcher's decisions; only this one checks the real tap on Chrome's helpers and the mic Chrome uses.
