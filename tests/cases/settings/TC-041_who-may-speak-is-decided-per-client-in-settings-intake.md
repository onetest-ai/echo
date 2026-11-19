---
id: TC-041
title: Who may speak is decided per client in Settings → Intake, with Intake off, and the Inbox's sender item opens it
priority: high
type: functional
requirements: [F-KNOW-04]
module: settings
run: agent
platform: any
data: standard
tags: [settings, intake, sender]
---

# TC-041: Who may speak is decided per client in Settings → Intake, with Intake off, and the Inbox's sender item opens it

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}`; every `echoctl` command runs after `. <dir>/env`
- Intake is off (the data's default); `recall/senders.json` holds, for Acme: Dana Lee (dana@acme.example) waiting, Acme News (news@acme.example) trusted and with 2 messages not authenticated

**Changes:** `<data>/recall/senders.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Navigate to `{{base_url}}/#today`, wait for "Your day", click "Inbox"; on the item "Acme News's mail isn't authenticated" click "Open" | The window shows Settings with "Intake" selected in the left list |
| 2 | Read the card "Who may speak" | A line says mail and posts are read only from you, colleagues and the senders you trust; a table headed "Acme · senders · 1 waiting · 1 not authenticated" lists Dana Lee (dana@acme.example) with "Trust" and "Ignore", Acme News with "Read anyway" and "Keep out", and Acme News "trusted" with "Forget" |
| 3 | Click "Read anyway" on Acme News | Within 2 s the row with "Read anyway" is gone; Acme News's trusted row says "trusted · unauthenticated mail read too" |
| 4 | Run `. <dir>/env && echoctl intake senders` | Under `acme`, `trusted` has `news@acme.example` with `"unverified": true`; no `unverified` entry is left |
| 5 | Click "Trust" on Dana Lee | Within 2 s Dana Lee's waiting row is gone; a row Dana Lee "trusted" with "Forget" shows; the head no longer says "waiting" |

## Expected Final State
Acme News's unauthenticated mail is read, Dana Lee is trusted.

## Teardown
- None (the data is fresh for each scenario).

## Notes
- Who may speak shows whether Intake is on or off: a thread's mail is read into its work either way, by the same senders list.
