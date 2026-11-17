---
id: S-37
title: "A plugin hears your microphone: on a click in its view with Echo's sign, stopped every way, refused without a click or the sheet, and the native window's grant"
run: agent
data: small
cases:
  - tests/cases/plugins/TC-130_a-plugin-that-asked-opens-the-microphone-on-a-click-in-its-view-with-echos-sign.md
  - tests/cases/plugins/TC-133_a-plugins-microphone-is-refused-with-the-reason-and-nothing-opened-without-a-click-or-without-the-sheet.md
  - tests/cases/plugins/TC-132_a-plugins-microphone-stops-on-stop-its-own-close-its-tracks-ending-leaving-its-view-and-the-plugin-switched-off.md
  - tests/cases/plugins/TC-131_in-the-native-window-a-plugin-gets-the-microphone-only-for-an-opening-echo-allowed.md
---

# S-37: A plugin hears your microphone

**Story:** Sam installs a voice plugin that asked for the microphone. It hears it only after Sam clicks in its own view, Echo's recording sign says so while it listens, and it stops whichever way Sam leaves it; a plugin that didn't ask, or one asking with no click, gets nothing.

**Data:** `small`: `tests/fixtures/serve.sh up <dir> <port> --set small`.

**Length:** about 4 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | 1–3 |
| The browser's stand-in microphone on this scenario's tab (`plugins/TC-130`'s preconditions) | the prelude | 1–3 |
| The marketplace `acme-local` with Acme voice and Acme plain, both installed | stage 1 (its precondition lines and step 1) | 1–3 |

## Prelude
1. Open a new tab for this scenario; load `{{base_url}}/`.
2. Run `plugins/TC-130`'s stand-in microphone snippet (`browser_run_code_unsafe`) on that tab.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-130_a-plugin-that-asked-opens-the-microphone-on-a-click-in-its-view-with-echos-sign.md` | — | Its precondition lines but the stand-in (the prelude's). Its teardown: only "Hang up" — the plugins and the marketplace stay for stages 2–3 |
| 2 | `plugins/TC-133_a-plugins-microphone-is-refused-with-the-reason-and-nothing-opened-without-a-click-or-without-the-sheet.md` | 1 | Its preconditions are stage 1's: skip them. Skip its teardown |
| 3 | `plugins/TC-132_a-plugins-microphone-stops-on-stop-its-own-close-its-tracks-ending-leaving-its-view-and-the-plugin-switched-off.md` | 1 | Its preconditions are stage 1's: skip them. Last: it switches Acme voice off; its teardown removes both plugins and `acme-local` |
| 4 | `plugins/TC-131_in-the-native-window-a-plugin-gets-the-microphone-only-for-an-opening-echo-allowed.md` | — | **person**, at the release, on macOS and on Windows |

## Notes
- The served window's browser never hears the computer's microphone: the prelude's stand-in answers the page's `getUserMedia` with a tone and counts the asks (`window.__gum`), so a refusal shows the device was never asked for. The tab is closed at the scenario's end, taking the stand-in with it.
- Stage 2 comes before stage 3 because stage 3 switches Acme voice off.
