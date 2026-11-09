---
name: doctor
description: "Use when an Echo recording comes out silent or empty, Echo isn't running, capture or minutes don't work, or a command says \"run the doctor skill\". Checks Echo end to end and fixes what's missing: the app, data folder, speech model, permissions, languages, Claude Code and where minutes are written."
---

# Echo doctor

CLI: `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl` (below: `echoctl`). Never record here, never toggle a
permission for the user (open the page and explain), and never download a model without a yes.

1. `echoctl up` (exit code 127 = Echo isn't installed: macOS, open `Echo-<version>.dmg` and drag
   Echo to Applications; Windows, run `echo-<version>.msi`; then retry).
   It starts the app if needed and reports `failing` and `next`. "nothing" in `next` → tell the
   user Echo is fine in one line and stop (asked about a call's minutes: step 6 first). If the app
   won't start, show the last lines of `<home>/.echo/app.log`.
2. **Data folder** (`echoctl app` → `home_source: default`): ask where calls should live
   (default `~/Echo`), then `echoctl home set <path>`.
3. **Permissions** belong to the app, not the terminal: `echoctl doctor --json --no-ai` (the AI
   part loads models: step 6 checks it once).
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
   end of `<call>/claude.log` say why; `echoctl attention` lists minutes that failed or wait, and
   what they wait for. `echoctl notes <call>` writes them now.
   Minutes need Claude Code: `echoctl claude` → its `check`. No `path` or an `error`: Claude Code
   isn't installed or doesn't start (https://claude.com/claude-code). `terminal.ok` false:
   `echoctl claude path`. `plugin_installed` false: `echoctl claude connect`.
   A local AI profile (a model on Ollama): `echoctl doctor --json` → `ai`; tell the user each of
   its `warnings` (the server gives the model too little context: long calls' minutes miss parts;
   `OLLAMA_CONTEXT_LENGTH` is set on that server, not in Echo) and `problems` (the model isn't
   there or couldn't load); for each model in `unreported` (this Ollama doesn't show the context),
   suggest updating Ollama, or setting `OLLAMA_CONTEXT_LENGTH` on that server to be safe. This
   loads each local model for up to 2 minutes: run it once, with a Bash timeout of 2 minutes per
   local model (minutes and live-notes models count separately) plus one.
7. Only if the user asks:
   - calendar: `echoctl calendar login` (read-only Microsoft 365; another org needs its own app
     registration → `echoctl config --graph-client-id <id>`);
   - automatic minutes (Echo writes them 15 min after a call, no session needed):
     `echoctl config --auto-minutes true|false`;
   - the folder Echo runs Claude Code in for minutes (with the built-in AI profile its
     `.claude/settings.json` applies, e.g. a local model): `echoctl config --notes-in <folder>`
     (`any` = the data folder again);
   - minutes model: `echoctl ai set <profile> --notes-model <name>` (the built-in profile `system`:
     `default` = Claude Code's own), or menu → AI for calls → Minutes model;
   - the plugin in another project: `echoctl claude connect --scope project --project <repo>`.
