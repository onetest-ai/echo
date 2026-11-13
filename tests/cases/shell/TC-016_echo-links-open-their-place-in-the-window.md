---
id: TC-016
title: An echo:// link opens its place in the window: a call at a moment, a thread, a client, a person, a task, a prep; an unknown one says so
priority: medium
type: functional
requirements: [F-WS-08]
module: shell
run: agent
platform: any
data: standard
tags: [shell, links]
auto: [tests/auto/specs/S-28.spec.mjs]
---

# TC-016: An echo:// link opens its place in the window: a call at a moment, a thread, a client, a person, a task, a prep; an unknown one says so

**Module:** Window shell | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard` (`data: standard`); the window is served at `{{base_url}}` and `{{base_url}}/` was loaded first (the cookie); every `echoctl` command runs after `. <dir>/env`
- A link is opened the way the app opens one the system hands it (`window/mod.rs`: the page is loaded at `#link=<the link, URL-encoded>`): navigate to `{{base_url}}/#link=` + `encodeURIComponent(link)`. Each step is a fresh navigation (`browser_navigate`, a full load)
- `A=$(ls "$ECHOCTL_HOME/calls" | grep -- '-acme-weekly-sync$' | tail -1)` (today's Acme weekly sync)

## Test Data

| Link | Opens |
|---|---|
| `echo://call/$A?t=43` | `#call/$A/transcript`, scrolled so the turn at 0:43 is in the middle |
| `echo://thread/acme-rollout` | `#project/acme-rollout/brief` |
| `echo://client/acme` | `#client/acme/overview` |
| `echo://thread/acme-rollout/profile` | `#project/acme-rollout/profile` (a thread on one of its tabs) |
| `echo://client/acme/profile` | `#client/acme/profile` |
| `echo://thread/acme-rollout/overview` | a notice, nothing opened (not one of a thread's tabs) |
| `echo://person/dana@acme.example` | `#project/acme-renewal/people` (the first thread she is placed in: Acme renewal, then Acme rollout) with Dana Lee's panel |
| `echo://task/acme-rollout/T001` | `#project/acme-rollout/work` with T001's Details |
| `echo://prep/e11de173ab857a8b` | `#upcoming/e11de173ab857a8b` (Globex sponsor update, still to come) |
| `echo://prep/1acb47358ad30580` | `#call/<today>_<time>-umbrella-intro-call/prep` (a recorded meeting of today; `<today>` is the build day, `YYYY-MM-DD`) |
| `echo://thread/no-such-thread` | a notice, nothing opened |

## Steps

| # | Action | Expected Result |
|---|---|---|
| 1 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fcall%2F<A>%3Ft%3D43`, wait 2 s, then run `browser_evaluate` with `() => { const p = document.querySelector('#pane').getBoundingClientRect(); const t = [...document.querySelectorAll('.turn')].find((e) => e.dataset.t.startsWith('43.')); const r = t.getBoundingClientRect(); return { hash: location.hash, h: document.querySelector('main h1, main h2').innerText, inView: r.top >= p.top && r.bottom <= p.bottom } }` | `hash` is `#call/<A>/transcript`; `h` is "Acme weekly sync"; `inView` is `true`: the turn at 0:43 (Dana Lee) is in view in the transcript pane |
| 2 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fthread%2Facme-rollout` | The hash becomes `#project/acme-rollout/brief`; the thread "Acme rollout" is open on its Brief tab ("Rolling out single sign-on and the data migration for Acme, Berlin first.") |
| 3 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fclient%2Facme` | The hash becomes `#client/acme/overview`; the client Acme's Overview: its About ("A manufacturer of everything, 4,000 people across Europe.") and under THREADS, among them, "Acme renewal" (Negotiation) and "Acme rollout" (Active) ("Acme hackathon" (Cancelled) is listed too) |
| 4 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fperson%2Fdana%40acme.example` | The hash becomes `#project/acme-renewal/people` (a person link opens the first thread the person is placed in, `links::person_thread`; Dana Lee is in Acme renewal and Acme rollout); the right panel shows "Person" with "Dana Lee", "Delivery manager · Acme" and `dana@acme.example · Acme` |
| 5 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Ftask%2Facme-rollout%2FT001` | The hash becomes `#project/acme-rollout/work`; the right panel shows "Details" with "T001", "task", the title "Send the SSO checklist" and Status "done" |
| 6 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fprep%2Fe11de173ab857a8b` | The hash becomes `#upcoming/e11de173ab857a8b`; the heading "Globex sponsor update" with its one tab, Prep; the Prep shows "IN SHORT" and "Globex sponsor update: the SSO checklist is done, the questionnaire waits for legal, the access list is late." |
| 7 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fprep%2F1acb47358ad30580` | The hash becomes `#call/<today>_<time>-umbrella-intro-call/prep`; the heading "Umbrella intro call" with the Prep tab selected |
| 8 | Navigate to `{{base_url}}/#link=echo%3A%2F%2Fthread%2Fno-such-thread` | The hash becomes `#`; a notice reads "Echo can't open that link: no thread \"no-such-thread\"" with "OK" |

## Expected Final State
Each link opened its place; nothing changed.

## Notes
- Step 8 checks #168: after an unknown link the window once landed on Calls and opened the first listed call; now it shows the notice and opens nothing (the calls list with "Pick a call").
- The native window opens `echo://` URLs the system gives it by loading the page at `#link=…` (`window/mod.rs`, the `Some(c) if c.starts_with("echo://")` branch); the served window takes the same route, so this case covers the routing. The system handing the URL to Echo (the registered scheme, a click in another app) needs the installed app: person-run.
- Step 1 checks where the jump leaves the transcript, not the mark: the turn gets `class="turn hit" id="jump"` for one render only (`render()`: `#jump` is scrolled into view with `block: 'center'`, then `S.jump` is cleared and the next redraw drops the mark). The lasting effect is that the 0:43 turn is in view; whether the pane had to scroll for it depends on the window's height and the transcript (on the standard data at 1201x868 it is already in view), so the case asks only that it is in view.
- A link inside the minutes or a Brain view goes through the same `openLink` (`window.echo.openLink`).
