---
id: TC-012
title: Open minutes in browser and Show in Finder use the call's files
priority: low
type: functional
requirements: [F-CALLS-17]
module: window
run: agent
platform: macos
tags: [minutes, native]
external_id: TC-WIN-003
auto: [tests/auto/specs/S-17.spec.mjs]
---

# TC-012: Open minutes in browser and Show in Finder use the call's files

**Module:** Window | **Priority:** Low | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (or `--ai local`, or a standard set): its `<dir>/bin/open` stands in for macOS's `open`, writes each call to `<dir>/open.log`, and with `ECHOCTL_TEST_OPEN_LOG_ONLY=1` opens nothing (no browser tab or Finder window on the tester's screen)
- The window served on that data with clicks reaching it and that switch: `. <dir>/env; ECHOCTL_TEST_OPEN_LOG_ONLY=1 ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve <port> > <dir>/serve.log 2>&1 &` (or `serve.sh up` for an S-17 run whose stages use it); the page loaded from `{{base_url}}/` first
- Acme weekly sync has minutes (test data); no minutes page written yet

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `#calls`, click the row `[role=option][data-call$="acme-weekly-sync"]`, then `[aria-label="Call actions"]` | The menu lists Show folder, Open minutes in browser, Show minutes in Finder, Write minutes again, Default, Discussion, Edit templates…, Ask Claude about this call, Delete… |
| 2 | Click "Open minutes in browser" | `<dir>/open.log` gets the line `<the call's folder>/Acme weekly sync-minutes.html`, and that file now exists in the call's folder (written on demand) |
| 3 | "Call actions" → "Show minutes in Finder" | `open.log` gets `-R <the call's folder>/Acme weekly sync-minutes.html` (Finder with the file selected) |
| 4 | "Call actions" → "Show folder" | `open.log` gets `<the call's folder>` |

## Expected Final State
Each menu item hands macOS the call's own file or folder; the minutes page is written when first asked for.

## Notes
- `open <file>` shows it in the default browser, `open -R` selects it in Finder: macOS's part. A person can look once that the tab and the window appear (drop `ECHOCTL_TEST_OPEN_LOG_ONLY`).
- Run 2026-11-08 evening (S-17, CLI part: the three commands sent to the served window's `/ipc` as the menu sends them — `open_minutes`, `reveal_minutes`, `reveal`): the three lines in `open.log`, the page written at the first; the menu itself not run.
- Run on 2026-11-07 (S-17): PASS — the three lines in `open.log`, the page (8.9 KB) written at the first click.
