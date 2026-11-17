---
id: TC-131
title: In the native window a plugin gets the microphone only for an opening Echo allowed, the tray shows it, a page's own getUserMedia and the camera are refused, and a recording that starts takes it away
priority: high
type: functional
requirements: [F-PLUG-86, F-PLUG-87, F-PLUG-88]
module: plugins
run: person
platform: any
data: none
tags: [plugins, client-half, microphone, privacy, native-window, release]
---

# TC-131: The native window's microphone for a plugin (macOS and Windows)

**Module:** Plugins | **Priority:** High | **Type:** Functional — a person, at the next release, on macOS and on Windows

## Preconditions
- The release build installed as the user's Echo (or a dev build run as the app), Echo allowed the microphone (macOS: System Settings → Privacy & Security → Microphone; Windows: Settings → Privacy → Microphone → "Let desktop apps access your microphone")
- `plugins/TC-130`'s marketplace `acme-local` added and **Acme voice** installed (its marketplace lines and step 1, with the user's own `echoctl`; not its browser stand-in: here the microphone is the computer's), the Echo window open; Settings → Plugins shows Acme voice on
- The window's developer tools (a dev build: right click → Inspect) for steps 3 to 5

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Open Acme voice's activity; click "Talk" | No system prompt (Echo already has the microphone); "open 1 live" on the page (`document.body.dataset.acmeMic`); the top pill "Acme voice hears your microphone" with Stop; the tray icon turns orange and its menu's first line reads "● Acme voice hears your microphone"; macOS also shows its own orange microphone dot in the menu bar |
| 2 | Click Stop in the pill | Within a second the tray icon is back to idle, its first line "Not recording"; macOS's orange dot goes |
| 3 | In the developer tools' console, on Acme voice's page: `navigator.mediaDevices.getUserMedia({audio: true}).then((s) => 'opened ' + s.getAudioTracks()[0].readyState, (e) => e.name)` | `NotAllowedError`: Stop took the grant back, so a page's own call outside an opening is refused; no orange dot |
| 4 | Click "Talk" (the pill shows); in the console `window.__acmeTrack.stop()`; wait 3 s (the pill goes); then step 3's `getUserMedia` line again | `NotAllowedError`: a plugin that stopped its own tracks leaves no grant behind; no orange dot |
| 5 | Same console: `navigator.mediaDevices.getUserMedia({video: true}).then(() => 'opened', (e) => e.name)` | `NotAllowedError`: the camera is never granted |
| 6 | Click "Talk" again (the pill shows); then start a memo from the tray ("Memo") | The memo starts; the plugin's microphone stops at once: the pill becomes the memo's, the page says "Echo started recording, so the plugin's microphone stopped" |
| 7 | With the memo still recording, click "Talk" | Refused: "refused: Echo is recording — a plugin gets the microphone once the recording stops"; nothing opens. Stop the memo |
| 8 | Click "Talk"; then close the Echo window | The tray icon goes back to idle; macOS's dot goes |

## Expected Final State
No plugin hears the microphone; the memo is saved as Echo's own.

## Teardown
- Delete the test memo in Calls; `echoctl plugin remove acme-voice --delete-data`; `echoctl plugin marketplace remove acme-local`

## Notes
- Written from the features' text (F-PLUG-86, F-PLUG-87, F-PLUG-88), not from the code.
- A person's check: the native webview (WKWebView, WebView2), the tray and the system's own microphone indicator can't be driven by the agent; the served window's part is `plugins/TC-130`, `plugins/TC-132` and `plugins/TC-133`'s.
- Windows: Echo's WebView2 handler (`SavesInProfile` off) is only type-checked on a Mac; this case on Windows, at the release, is its first run there.
- Screen sharing is not Echo's to grant or refuse (F-PLUG-88), so no step checks it.
