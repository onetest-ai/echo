---
id: S-43
title: "A plugin on a call's page: its chip on the header with its own popup, its menu items named from the call, its line while the minutes come, its sheet, and its activity's badge and kept box"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-180_a-plugins-chip-on-a-calls-header-with-its-own-popup-and-its-record-per-call.md
  - tests/cases/plugins/TC-181_a-plugins-items-in-a-calls-menu-are-named-from-the-call-and-grouped-per-plugin.md
  - tests/cases/plugins/TC-182_a-plugins-line-in-the-minutes-box-only-while-the-minutes-are-coming.md
  - tests/cases/plugins/TC-183_a-plugins-sheet-opens-over-the-window-and-closes-on-esc-a-click-beside-it-its-own-button-or-its-plugin-going.md
  - tests/cases/plugins/TC-184_a-plugins-activity-shows-a-badge-and-keeps-its-box-while-you-type-in-it.md
---

# S-43: A plugin on a call's page

**Story:** Sam installs a plugin that files his calls in his notes. On a call's header it puts a chip — "Not filed" — whose own popup files the call; the call's ⋯ menu offers "Open in Acme notes" for an Acme call and "Open in Inbox notes" for another, beside another plugin's item in a group of its own. While a call's minutes are on their way, the plugin says where they will go. "Open in…" opens its sheet over the window: Sam tabs through it, closes it with Esc or a click beside it, or files the call. Its own activity counts what he filed, and doesn't lose what he types while the plugin keeps redrawing.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`. Browser 1201 × 868.

**Length:** about 10 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build | the lead (`serve.sh up … --set standard`) | 1–6 |
| The test marketplace with Call demo and Menu demo installed | stage 1 (TC-180 step 1) | 1–6 (removed after stage 6) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-184_a-plugins-activity-shows-a-badge-and-keeps-its-box-while-you-type-in-it.md` step 1 only | — | Run TC-180 step 1 (install), load `{{base_url}}/`: the badge is "!" before anything is filed |
| 2 | `plugins/TC-180_a-plugins-chip-on-a-calls-header-with-its-own-popup-and-its-record-per-call.md` steps 2–8 | 1 | — |
| 3 | `plugins/TC-181_a-plugins-items-in-a-calls-menu-are-named-from-the-call-and-grouped-per-plugin.md` | 2 | — |
| 4 | `plugins/TC-182_a-plugins-line-in-the-minutes-box-only-while-the-minutes-are-coming.md` | 1 | — |
| 5 | `plugins/TC-183_a-plugins-sheet-opens-over-the-window-and-closes-on-esc-a-click-beside-it-its-own-button-or-its-plugin-going.md` | 1 | — |
| 6 | `plugins/TC-184_a-plugins-activity-shows-a-badge-and-keeps-its-box-while-you-type-in-it.md` steps 2–5 | 2 | Its step 2 is done by stage 2: read the badge (the number of calls filed) |

## Notes
- Stage 4 runs while Globex standup's minutes are still ahead (the data's "Minutes in about 15 min", moved by its notes): run it within ten minutes of the data build, or build it again.
- Teardown after stage 6: TC-180's.
