---
name: doctor
description: Check Echo and fix what's missing — the app not installed or not running, no data folder or speech model, missing permissions (microphone, system audio), languages, calendar, where and when minutes are written. Use when a recording comes out silent or empty, Echo isn't running, the user asks why capture or minutes aren't working, or a command says "run the doctor skill".
---

# Echo doctor

The app sets itself up on first launch (data folder, speech model, start at login,
permissions); this skill finds what's still missing and fixes it. CLI:
`${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Never record here, never toggle a
permission for the user (open the page and explain), and never download a model without a yes.

1. `echoctl up` (exit code 127 = the app isn't installed: macOS, open `Echo-<version>.dmg` and
   drag Echo to Applications; Windows, unzip to `%LOCALAPPDATA%\Programs\echo`; wait, retry).
   It starts the app if needed and reports `failing` and `next`. "nothing" in `next` → tell the
   user Echo is fine in one line and stop (asked about a call's minutes: step 6 first). If the app
   won't start, show the last lines of `<home>/.echo/app.log`.
2. **Data folder** (`echoctl app` → `home_source: default`): ask where calls should live
   (default `~/Echo`), then `echoctl home set <path>`.
3. **Permissions** belong to the app, not the terminal: `echoctl doctor --json`.
   - `os_supported` false → say `os_requirement` and stop.
   - Each check not `granted`: explain `why` in a sentence. `not_determined` →
     `echoctl doctor --request` and tell them to click **Allow**; `denied` →
     `echoctl doctor --open <id>` and walk them through `how`. Accessibility is optional (mute
     sync, speaker names) and needs `echoctl app quit && echoctl app start` after.
   - Repeat until `ready`; on macOS finish with `echoctl doctor --probe` while audio plays (the
     only reliable proof that System Audio Recording works).
4. **Speech model** (none `active` in `echoctl model list --json`): offer the `recommended` one
   with its `reason` and `size_mb`, plus one larger and one smaller; on a yes,
   `echoctl model download <name>`.
5. **Languages**, if the transcript came out in the wrong language: `echoctl config --languages
   en,uk` (detection is limited to this list).
6. **Minutes** missing or failed for a call: `<call>/notes-run.json` (`state`, `message`) and the
   end of `<call>/claude.log` say why; `echoctl notes <call> --auto --for .` prints when (or why
   not) Echo writes them by itself. `echoctl notes <call>` writes them now.
7. Only if the user asks:
   - calendar: `echoctl calendar login` (read-only Microsoft 365; another org needs its own app
     registration → `echoctl config --graph-client-id <id>`);
   - automatic minutes (Echo writes them 15 min after a call, no session needed):
     `echoctl config --auto-minutes true|false`;
   - the folder Echo runs Claude Code in for minutes (with the built-in AI profile its
     `.claude/settings.json` applies, e.g. a local model): `echoctl config --notes-in <folder>`
     (`any` = the data folder again);
   - minutes model: `echoctl config --notes-model <name>` (`default` = Claude Code's own);
   - the plugin in another project: `echoctl claude connect --scope project --project <repo>`.
