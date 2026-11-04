# Echo

A standalone app (Rust: tray icon + call watcher + CLI) that records Teams calls the user opts in
to (their mic + the Teams app's audio, as two tracks) and transcribes them on-device, plus a
Claude Code plugin that is its client (a status line, minutes, follow-ups). **Separation of concerns:**
the app never depends on Claude Code (it asks with native dialogs, starts at login, has its own
permissions); the plugin drives it only through the CLI and the files in the data folder.
The final notes/actions (`notes.md`, `actions.json`) are written only by a Claude Code run (the app writes
transcripts, never notes). One exception, by design: the app saves model-written **live drafts** during a call
(`live-notes.json`, `live-asks.jsonl`, `livenotes.rs`); the minutes get the live notes as a checklist
(`context.live_notes`: decisions, actions, questions with their times, the stretch covered and the gaps) — each
checked against the transcript, which wins (the user's call, 2026-10-30: "these notes are very valuable for the
minutes").

## Hard requirements
- macOS **and** Windows. Platform code lives behind `#[cfg]` in `audio/{macos,windows}.rs` and
  `permissions/{macos,windows}.rs`; shared code must not use platform APIs.
- Recording is opt-in per meeting (join-and-record), never automatic for every call.
- Transcription is on-device (whisper.cpp via whisper-rs, or NVIDIA Nemotron via NeMo-Speech.cpp;
  `transcribe` feature), or on a transcription server the user connects to themselves
  (`remote.rs`, below). Never a cloud service by default.

## Layout
- `crates/echoctl` — one binary: the app (`app run`, or no arguments inside Echo.app /
  from Explorer) and the CLI: `app [start|quit|autostart|install-cli]`, `home [set|choose]`,
  `claude [connect --scope]`, `doctor`, `procs`, `capture`, `model list|download|use|remove`,
  `config` (incl. `--notes-in`), `transcribe`, `pending --for`, `claim --for`, `ai`, `live`, `prep`, …
  `app.rs` = app lifecycle (single instance via the `watch.json` heartbeat, first run, control
  requests, native dialogs/notifications, autostart: LaunchAgent / HKCU Run key).
- Data: `paths.rs`. Config dir (fixed): macOS `~/Library/Application Support/echo`, Windows
  `%APPDATA%\echo` → `home.json` (chosen data folder) + `models/` (shared; old
  `<home>/.echo/models` still used). Home = `ECHOCTL_HOME` → `home.json` → config dir.
  Never the cwd or a Claude workspace.
- `packaging/macos/` (Info.plist, entitlements) + `scripts/package-macos.sh` (Developer ID,
  hardened runtime, notarize + staple the app, signed (not notarized) dmg; notary creds via `APPLE_API_KEY`/`_KEY_ID`/`_ISSUER` (API key) or keychain profile `echo-notary`).
  `LOCAL=1`: no developer account — ad-hoc signature, no notarization or dmg, plus
  `disable-library-validation` (an ad-hoc signature has no Team ID, and the hardened runtime would
  refuse the NeMo-Speech libraries). TCC keys permissions to the signature: re-granted per build.
  `scripts/package-windows.ps1` (unsigned for now): a per-user MSI (`packaging/windows/echo.wxs`, WiX 5 —
  later WiX versions carry a maintenance-fee EULA; the NuGet tool package in `%LOCALAPPDATA%\wix5-tool` needs no
  admin) → `%LOCALAPPDATA%\Programs\echo`, Start menu shortcut (AUMID `onetest.Echo`), user PATH, Run key;
  `app quit` before files are replaced (its exit code checked when an installed `echoctl.exe` is found through
  HKCU `Software\onetest\Echo\InstallFolder`: a quit the user cancelled — notes unsaved in the window — or one that
  took over 30 s stops the installation; else ignored, as for a first install, and for Echo's own update, which passes
  `ECHO_SELF_UPDATE=1`: it saved its notes and is stopping already), `app start` after; uninstall keeps data. Plus the
  same files as a zip. CI: `.github/workflows/windows-release.yml` builds it on GitHub's runner when a release is
  published (or by hand for a tag) and uploads both; nothing else builds (changes pile up on `main`, a
  release ships them) and nothing is cached — every release builds from scratch, ~25 min; `-Phase stage` /
  `package` split the script so a code-signing step can sign `echoctl.exe` in between, then the MSI. Unsigned for now:
  SignPath's free open-source signing wasn't granted (2026-11-03); a certificate will be bought later, and the site's
  /windows/ page says what users see meanwhile (SmartScreen: More info → Run anyway).
- Marketplace `echoctl` (`.claude-plugin/marketplace.json`): `claude plugin marketplace add
  onetest-ai/echo` + `claude plugin install echo@onetest --scope project|user|local` (project = the data folder, the default)
  (what `echoctl claude connect` runs; `ECHOCTL_MARKETPLACE` overrides the source).
- CLI on PATH (`app install-cli`): macOS `/usr/local/bin/echoctl` → the app's binary (admin
  prompt via osascript); Windows: app folder appended to the user PATH.
- `plugin/` — the Claude Code plugin; ships no binary: `plugin/scripts/echoctl[.cmd]` finds the
  CLI (`$ECHOCTL_BIN` → `plugin/bin` dev build from `scripts/build-plugin.sh` → the app's own
  path in `<config dir>/app-path` (written at each app start: any install folder) → PATH →
  Echo.app / `%LOCALAPPDATA%\Programs\echo`). On Windows Claude Code's Bash is Git Bash, so
  the skills run the **sh** launcher there too (it handles MINGW paths via `cygpath`). Skills:
  `manage` (the CLI for clients, organisations, threads and their rules, the work ledger, people, calls, mail through
  `deliver`, the Brain — any session; `<echo_howto>` in Echo's window points to it; descriptions follow Anthropic's
  skill best practices: third person, what it does, then "Use when…"), `process-call` (notes → actions → vocabulary →
  follow-ups), `record` (arm / stop from chat), `doctor` (`echoctl up` + fix what's missing: data folder,
  permissions, model, languages; first run itself is the app's). The user's own notes during a
  call (`my-notes.md`) come from the window's Notes tab (during and after a call) or the tray's "Add a note…" while
  recording (`echoctl note`).
  Note templates: `crates/echoctl/templates/*.md`, compiled in (user overrides in `<home>/.echo/templates/`).
  Post-call facts come from `echoctl context <call>` (gate: ≥ 3 min and ≥ 1,500 chars, or any saved frames; heading
  budget by length; colleagues vs external by email domain; previous notes of a recurring meeting).
- `<home>/calls/<ts>-<slug>/` — `meta.json`, `transcript.md`, `segments.json`, `notes.md` (git-ignored). `mic.wav`/`remote.wav`
  exist only until transcription succeeds, then are deleted (`config.keep_audio`, default false) — text only.
- `<home>/.echo/` — `config.json` (model, languages, vocabulary), control files
  (`armed.json`, `stop.request`, `quit.request` → `quit.asking` / `quit.cancelled`, `doctor.request` →
  `health.json`), `events.jsonl`,
  `watch.json` (heartbeat), `agenda.json`, `app.log` (background launches)
- `<call>/meeting.json` — optional context (subject, attendees). Deterministic, no model:
  `agenda sync` reads today's events from Microsoft 365 through the app's own Graph client
  (`graph.rs`: auth code + PKCE in the system browser, `http://localhost` loopback — keeps
  Conditional Access device claims; read-only `Calendars.Read`; app registration =
  `config.graph_client_id`, default CodeMie's public client (an open project's registration); tokens in
  `<config dir>/graph.json` 0600; `echoctl calendar login|logout|events`). Not signed in →
  the msgraph skill CLI if installed, else a quiet no-op. Teams' own IndexedDB calendar cache
  was rejected (undocumented, "data from other apps" prompt). Events are filtered into `.echo/agenda.json`;
  the watcher re-syncs every 15 min and attaches the overlapping meeting when a recording starts.
- Rule of thumb: anything deterministic (matching, filtering, template choice, preflight) is a CLI
  command (`up`, `agenda sync`, `context`); skills keep only judgment (notes, titles, vocabulary
  picks, follow-ups); the app does everything else.

## Platform notes
- macOS 14.2+: Core Audio process tap on all `com.microsoft.teams2*` processes + private
  **tap-only** aggregate device (no output sub-device). Tap-only = clocked by the tap at a fixed
  48 kHz and immune to output switches; Apple's sample layout (output device as clock) runs at
  the headset's rate — 24 kHz for AirPods in call mode. `ECHOCTL_TAP_CLOCK=output` restores it.
  In a call, `com.microsoft.teams2.modulehost` shows input+output running.
  TCC permissions attach to the responsible app: Echo.app when the app records (launched
  via LaunchServices: `open`, login item); the terminal when the CLI captures by hand. So
  `doctor` asks the running app (`doctor.request`) to check/prompt in its own process.
  `TCCAccessPreflight(kTCCServiceAudioCapture)` returns 2 (unknown) even when granted, and a
  denied tap still delivers frames — all zeros. The only reliable check is `doctor --probe`
  (1.5 s global tap, peak > 0 ⇒ granted). The first tap started is what triggers the prompt.
  `ECHOCTL_DEBUG=1` prints raw preflight / probe values.
- Windows 10 2004+: remote track = WASAPI **output-device loopback** of the device Teams plays on
  (its playback sessions; follows device switches; never while Teams isn't running). New Teams is
  blind to process loopback (exact zeros although its media process has an active playback
  session), so other sounds on that device are recorded too — said in the README and `doctor`.
  `ECHOCTL_LOOPBACK=process` / `ECHOCTL_LOOPBACK_PID` for diagnostics; `echoctl procs` lists
  capture and playback sessions. Call presence = an active capture session in Teams' tree.
  `ax/windows.rs` = UI Automation (mute sync, `ui.jsonl`): Teams windows via `EnumWindows` (UIA's
  desktop children wait seconds on hung windows), one cached subtree call per window; WebView2's
  tree appears only after the first request. Teams' mic button = AutomationId
  `microphone-button` ("Mute mic"/"Unmute mic"), in the call window (a separate window from the
  main one, which shows chats). `ax::CallWindow` finds it once (~60 ms) and keeps it (re-checked
  every 10 s); `ui.jsonl` and frames (`PrintWindow(PW_RENDERFULLCONTENT)`) read that window only.
  Per-tick queries are shared: Teams' process tree (one snapshot, 2 s) and its mic sessions
  (0.9 s) serve call detection and both tracks. Claude Code's Bash doesn't see the PATH a notes run gets: on Windows the prompt names
  the plugin launcher by its forward-slash path. A folder can't be renamed while a file in it is
  open: the notes run retitles the call folder after closing its log.
  Toasts: AppUserModelID `onetest.Echo` registered under HKCU. Build needs LLVM (`LIBCLANG_PATH`,
  bindgen only) and Rust ≥ 1.90. whisper.cpp build options live in `packaging/windows/whisper.cmake`
  (via `.cargo/config.toml`, MSVC target only): AVX2 not native (a native build crashes on CPUs
  without the build machine's AVX-512), `/O2` (the cmake crate drops it → /Od, ~10× slower), and
  `+crt-static` (no VC++ Redistributable). GPU (`gpu.rs`): a Vulkan driver crashed the process
  inside whisper.cpp (0xC0000409 at recording start, no discrete GPU), so the GPU is tried once
  per version in a child (`echoctl gpu-check`, at app start) → `<config dir>/gpu.json`; failed or
  died ⇒ CPU. `config.use_gpu` overrides; macOS always uses Metal. Tray → Health shows
  "Speech engine: …" (`gpu::status`, never runs the check) and, on Windows, Re-test GPU
  (`gpu::retest`: forget `gpu.json`, test again, notify). Release zips build `--features vulkan` (Vulkan SDK;
  short `--target-dir`: the shader generator's nested CMake hits MAX_PATH) and ship LunarG's signed
  `vulkan-1.dll`; no GPU driver ⇒ whisper.cpp falls back to the CPU. 11 s test call: CPU 49 s,
  Vulkan 12 s (a desktop with an NVIDIA GPU).
  What's missing or untested on Windows: `docs/windows-todo.md`.

## Capture model
`audio::Track` supervises one `Source` (mic / app tap / loopback): polls `health()` every 250 ms
and reopens on device change, rate change, stall, or app process-set change. The writer thread
resamples every incarnation to 16 kHz mono and pads gaps from a shared wall clock (`t0`), so
`mic.wav` and `remote.wav` stay aligned. Restarts are logged as `events` in `meta.json`.

## App flow
The app (`app run`; `watch` is a hidden old alias) runs from login or `app start`. Every
1 s: agenda (`.echo/agenda.json`) → `meeting_soon` at T−58 s…T+4 min; call presence = app
holds the mic (2 s release debounce); records only when `.echo/armed.json` exists (written by
`arm`/`join`, single-use, expires 15 min); stops on `stop.request` or 10 s after the call ended.
An arm has an `id`; writing, cancelling, expiring and taking one hold `.echo/armed.lock`. A recording takes (removes)
its arm as it starts, so an arm made during a recording — the next meeting's — survives its end and its Stop: Stop
ends the recording only, and an arm pending then waits for the next call when this one goes on (`held_arm`).
`echoctl stop` with nothing recording still cancels the pending arm.
Events → `<home>/.echo/events.jsonl`; clients follow them (the mod, or `events --follow`).
`config.scheduled` / `unscheduled` = `ask` → the app shows its own "Record?" dialog (osascript /
WScript popup: Yes/No on Windows, `app::yes_no` — Yes = the default button, or the last one when the
default is the first, Cancel / Not now); clients don't ask.
Windows detects calls from WASAPI: an active capture session owned by the Teams process tree
(`echoctl procs` lists them); that session's endpoint is the mic track's device.
Tray (`tray` feature, default; `app run --no-tray` to disable): the app's menu-bar/tray icon
(the onetest octopus simplified to a silhouette that reads at 18 pt — round head, two big
eyes, two arms waving up, four chunky flaring tentacles; no smile/spots/tuft — drawn from signed-distance shapes in
`mark.rs`; `assets/octopus.svg` is the same shape). Idle = template mark, armed = green,
recording = orange. The app icon comes from the onetest logo (`packaging/macos/onetest-logo-1024.png`):
`echoctl dev-icon dist/echo.icon --from …` writes an Icon Composer document (purple fill + the
octopus as one layer) that `actool` (Xcode 26) compiles into `Assets.car` (`CFBundleIconName`)
+ `echo.icns` — macOS 26 puts any other icon on a grey tile. No such actool → our own
rounded-square `.iconset`. Dev builds (no bundle) set the round logo as the Dock icon at
runtime. macOS: a Dock icon for as long as the app runs (`ActivationPolicy::Regular` at start; Echo.app stays
`LSUIElement`, so the CLI from the same binary never shows one) — a menu-bar icon can be hidden (no room beside the
notch) and Echo then looked gone; a click on it with no window showing opens the window (`Event::Reopen`). Settings →
General → Hide from the Dock when the window is closed (`config.hide_dock_icon`, off by default; macOS only): then the
icon shows only while the window is open (`tray::apply_dock`, AppKit's activation policy on open and close). Notifications (macOS) go
through UserNotifications as Echo.app (its icon; a click opens the window — "Update available" asks to
install instead, `app::Click`; asks permission at
first start); dev builds fall back to osascript, which macOS shows as Script Editor. Menu: status, Record, Stop,
recent calls, Settings (data folder, start at login, install CLI), Speech models (use / download
with % / remove), Claude Code (connect = install plugin at user scope; where Echo runs it for minutes; write minutes
automatically; minutes page), AI for calls (live notes, the profile new calls use with each one's minutes model, the
default's Minutes model…, sign-ins),
Health (Ask for permissions),
Check for updates… (or Update to <version>… once one is found), Quit — clicks call the same `arm`/`stop` as the CLI, so every client sees the same events.
The menu is rebuilt when what it shows changes (checked every 5 s); on macOS never while a menu is open: AppKit
tracks it in the event-tracking run loop mode, the event loop keeps running, and a menu replaced then closes under
the pointer (`tray::menu_open`; the rebuild comes on the first tick after it closes).
macOS needs the event loop on the main thread, so `run_loop` runs on a worker thread.
Prompts follow `config.scheduled` (calendar meetings) and `config.unscheduled` (other calls), each
`ask` | `manual`; with `manual` for both nothing prompts and the user arms explicitly.
`scripts/build-plugin.sh` must replace `plugin/bin/echoctl` by rename, never overwrite in place:
macOS SIGKILLs (exit 137) a binary rewritten in place.

## The Echo window (`window.rs`, `ui/`)
Menu → Open Echo… (or `echoctl app open [--call <name>]`, or a recent call) opens one window:
Calls (list + Minutes / Transcript / Chat / Screens / Actions — one Transcript tab: live while the call records, with a
recording dot and first, the final one after, its live notes beside it either way; `#call/x/live` opens it —, and **My notes** in the right panel), Calendar.
**A workbench** (`docs/plugins-design.md`): fixed areas — a 40 px top strip with the **Record pill** at its centre
(idle Record ▾ with language + profile, armed + Cancel, meeting soon + Record it, recording: clock, title, Stop that
asks first; "Writing the transcript…" beside it), the activity bar (`ACTIVITIES`: Calls `list-detail`, Calendar
`single`, and at its foot **Settings** — ⌘/Ctrl+, — `single`; ⌘/Ctrl+1…9), the primary view, right and bottom panels (`PANEL_VIEWS`, none yet: drawn only when a view is
in them; the first is **My notes**, right by default; the right panel starts **closed** every time the window opens and opens
when asked for — `Layout.started()`, the user 2026-11-04 —, in a narrow window over the page), a status bar. The call header's actions are one ⋯ menu (folder, minutes page, delete) plus the
My notes button; the Minutes tab's bar only says what is going on. Echo's own views are registry entries like a plugin's will be (`CALL_TABS`, `EVENT_ACTIONS`;
the block between `// ── workbench` and `// end of the workbench block` has no DOM: `ui/workbench.test.mjs`).
`Layout` keeps where panel views go and which panels are open in `<config dir>/window-layout.json` (per computer;
`/api/layout`, IPC `save_layout`, ≤ 16 KB; nothing else reads it). Below 900 px the calls list folds behind
"Calls ▾" and a right panel opens over the page; below 600 px of height the bottom panel folds. macOS: the title
bar is transparent with the content under it (`with_fullsize_content_view`); the page posts `window_drag` /
`window_zoom` from the strip (`window::window_cmd`, run by the tray on the window). Windows keeps its native title
bar for now. wry (Tauri's webview crate: WKWebView / WebView2) on the tray's tao
loop — not full Tauri (it would take over our loop, tray, packaging, updater). Content only via
the `echo://` protocol (`http://echo.localhost` on Windows): compiled-in `ui/index.html`,
`app.js`, `app.css` (vanilla, no npm/CDN; the octopus symbol is injected from
`assets/octopus.svg`), `/api/{state,calls,agenda,call/<name>}` JSON (`/api/calls?limit=&before=<name>&q=` = a page, newest first, `{calls, next, total}` — `calls_page`, 60 a page, ≤ 500; the list loads the next page near its end and searches title, people and subject in the app; a refresh reads the first page again and puts it on top of what is loaded; `patch` keeps a redrawn region's scrolled lists where they were; a new search starts at the top), `/file/<call>/frames/<img>`
after a path check; strict CSP; everything rendered is escaped (chat/transcripts are others'
text). **⌘K palette** (⌘K; Ctrl+K or Ctrl+Shift+P on Windows; status bar "Actions"; not while a terminal has the keys):
actions on a target — the open call, the selected calendar event, or none (a chip; ⌫ on an empty box drops it) —
Echo's own (Record / Stop… / Cancel, Claude, panels, updates, permissions) and Go to (activities, Settings sections,
calls by title, people or day). Unavailable ones stay listed, greyed, with why. Each action runs as the click on the
button that does it elsewhere (`press`), so it can't differ from it. Ranking (`Palette`, no DOM:
`ui/palette.test.mjs`): every typed word in the label (anywhere) or a keyword (word start), word starts and earlier
matches first, unavailable last, each group kept together. ⇧⌘R / Ctrl+Shift+R asks to stop a recording.
**Needs you** — shown as **Inbox** in the window since 2026-11-02 (the code and these notes keep "Needs you"; the panel id is `needs`) — (`attention.rs`, `/api/attention`, `echoctl attention`): what waits for the user's decision, collected
from where it lives (no model): a profile to sign in to (urgent: the default, or minutes waiting for it), minutes that
failed (14 days), senders waiting, people suggestions, Found for you, the Work tab's To review, topics or numbers to
accept, missing permissions, an update — each with the IPC message its own place sends (Trust, Yes, Create, Sign in…)
or the hash where it is decided. Not an activity: its entry sits above Settings at the foot of the activity bar (a
count; warn-coloured when one blocks work) and shows or hides the `needs` panel view (⇧⌘I; Ctrl+Shift+U on Windows,
where Ctrl+Shift+I opens WebView2's DevTools; ⌘K); the
places keep their own badges. A decision clicked here keeps its item, working (a spinner and what it does), until the item is gone: a background job's (Rebuild topics, Import — `/api/attention` `busy`, by item key; Update it too; its end — a panic too, `OnEnd` — makes the next request collect again, `attention::stale`, and a collection that ran across it isn't cached) for as long as it runs, any other up to 15 s; a job that failed gives the buttons back with why (`failed`: an import's error), a notice — the action couldn't be done — the last click's. An item gone (decided here or there) moves to Done for 7 days
(`<home>/.echo/attention.json`, the open items kept whole) — only when its source was read: a source that couldn't be
read now (or a sign-in not checked yet) keeps its items open as they were; one switched off (the Brain), an update
not checked yet, failed minutes older than 14 days leave quietly. Nothing to mark read. Collected at most every 20 s,
one collection at a time (`?fresh=1` after an action); `echoctl attention` only reads (a terminal sees neither the
app's sign-in checks nor its update check). The page shows ≤ 5 of a kind per group ("N more like these").
**Settings** (activity, `#settings/<section>`): General, Recording, Transcription, AI & minutes, Calendar & chat,
About me (name, email, and `me.md` — the profile the minutes read — shown rendered by `minutes::markdown_html`, the
minutes' safe writer, line by line, and edited in place: `settings_set me_profile`), Layout, Health (a ⚠ badge counts
required permissions missing). **Vocabulary** is a panel view (the book beside Calls' heading, ⌘K; the old
`#settings/vocabulary` opens it): the always-on terms and the learned ones with the meetings and people they are scoped
to (`learned_always` / `learned_remove`), saved as Settings' values are. `/api/settings` = one snapshot
(`window::settings_snapshot`: config values, models via `models::list_value`, downloads, the server, the calendar
account, permissions). Plain values save at once with the value shown — never a toggle of a stale one — through IPC
`settings_set {key, value}` (`window::settings_set`: config.json, or ai.json under its lock for `live_notes` /
`default_profile`); flows with a dialog, a sign-in or a download are the tray's own `Action`s, run by name
(`settings_action {action, arg}` → `tray::Ui::page_action` → `Ui::run`, the same code as the menu's).
**Claude in the window** (`terminal.rs`; xterm.js 5.5 vendored in `ui/vendor/`, see its README): a pseudo-terminal per session
(`portable-pty`: macOS PTY, Windows ConPTY) owned by the app — closing the window keeps it, quitting Echo ends it
(`close_all` on `UiEvent::Quit`). Output: a 2 MB ring per terminal, numbered by byte; the page long-polls
`/api/pty/<id>?from=<byte>` (≤ 25 s; a reopened window gets the scrollback), keys / size / close over IPC
(`pty_input`, `pty_resize`, `pty_close`; `pty_open {kind, key, call|profile}` → `window.echo.ptyOpened`). Each xterm
element is made once and moved into its panel slot after every render (a hidden holder otherwise), so renders and
panel moves keep it; keys inside `.xterm` all go to it (⌘/Ctrl+Esc gives focus back) except `termKey`'s: on macOS
⌘C copies the selection, ⌘A selects all and ⌘V pastes through the app (IPC `pty_paste` → `pbpaste`, bracketed when the
program asked; no text → Ctrl+V for Claude Code's image paste) — the window has no Edit menu, so WebKit's own never
runs —, and Ctrl/Shift+Enter send ESC CR (a new line in Claude Code's prompt). **One panel view now, "Claude"** (2026-11-04, the user: two sessions confused people): the Echo-folder session below;
"Ask Claude about this call" opens it, and the open call reaches it through `<echo_view>`. At its start it gets
`<echo_howto>` (`view::howto_block`): how to do Echo's things with this Echo's CLI — clients, organisations, threads,
work, calls — and mail through `deliver send mail --to` (Echo's own Microsoft 365 sign-in; the user confirms first;
not signed in → `compose`), not a Microsoft 365 connector of Claude's (2026-11-04: it tried an enterprise connector
that wasn't set up). Measured: a fresh session made a client and a thread, and for a mail checked `deliver status`.
The call's guarded session below is still in the app (`terminal.rs`, `pty_open {kind: call}`), no longer in the
window. Was: two panel views (bottom by default): **Claude · this call** — guarded on the call's own profile (`ai::for_call`; a sign-in problem comes back
with its profile → Sign in…): the minutes run's guard with `ECHOCTL_RUN_MODE=ask` (Read/Glob/Grep in the call and the
previous minutes, Bash only `echoctl context` — `guard::ASK_COMMANDS` —, no Edit/Write/Skill), `--tools
Bash,Read,Glob,Grep --strict-mcp-config --permission-mode default`, allow rules for reading the call, headless,
Claude Code ≥ `guard::MIN_CLAUDE`; one call session at a time — and **Claude · Echo folder**, the user's own Claude
Code (not headless) on the profile they pick (`ai::ready`, default = the default profile) — with `config.folder_skip_permissions` (Settings → AI & minutes → Claude in the window,
off by default) started with `--dangerously-skip-permissions`: in auto mode it can reach other sessions and systems;
never the call's session or a run. Both run in the data
folder (never a call's: Windows renames), hold their profile's lease while they run, start with `--session-id` and
resume with `--resume` (ids per profile in `<home>/.echo/sessions.json`: `<profile>:folder`, `<profile>:<stamp>`; one
refused within 4 s is forgotten). Entry points: status bar Claude, ⌘/Ctrl+`, the call's ⋯ → Ask Claude about this
call. **Restart and a new conversation** (#44): ⌘K actions per Claude panel (`claudeSessionActions`: Restart — the same id
resumed, plugins and settings read afresh; New conversation — a fresh `--session-id`, the old one kept in
`<home>/.echo/session-history.json`; `/api/conversations` still lists them, no UI reads it since the panel's own row was
dropped, 2026-11-03: "such a notification is enough"); `pty_open {session: '' | 'new' | <id>}` → `choose_session`.
`/api/ptys` `stale` = `{at, what}`: the profile's `plugins/installed_plugins.json`, or (folder) a plugin's session skills,
changed in content (Echo's `echo@…` entries; the skills' files hashed) since the session started → **Restart to update** on
the panel. Restart / New / Switch ask first while it runs (what it is doing is lost; the conversation is kept); the page
waits until the old session's processes are gone (`terminal::stopping`, `/api/ptys` `{key, stopping}`; a start of a key
still stopping is refused), and a session Echo stopped never forgets its conversation (`Term::stopped`; the refused-at-once
rule forgets only the id it started with, while still the panel's). `ai::terminal_command` = `claude_command`'s command and environment for a PTY (`ECHOCTL_DEBUG` prints its
arguments). `echoctl window --selftest` runs a shell in a terminal end to end (`terminal: true`).
**What the window shows, in each prompt** (`view.rs`): the page reports its view (activity; the open call — title,
time, tab, its threads; the thread and tab; the work item in the details panel; the calendar event; the panels; text
selected in the page or the minutes frame, `{echo: 'sel'}`) over IPC `view_state` when it changes and every minute →
`<config dir>/window-view.json`. Both terminal sessions' `--settings` (the call's guard file; `session-folder.json`
for the folder's) carry a `UserPromptSubmit` hook, `echoctl view-hook`, that adds it as `additionalContext` in an
`<echo_view>` block (what "this" / "here" mean; page text is data: no `<`/`>`, ≤ 400 chars each; a call's folder only
when it is one; nothing when older than 15 min), **only when it changed** since the last block that session got (it stays in the
session's history; per session id, a hash in `<config dir>/window-view-sent.json`, 50 sessions, 14 days) — a
`SessionStart` hook (`compact|clear`) forgets the session, so the next prompt gets it again. **Echo only**: it answers only with `ECHOCTL_TERMINAL=1`, set on the
Claude Code Echo's terminal starts — never the minutes or sync runs, the plugin, or the user's other sessions.
**My notes** (a panel view, `notesPanel`; it was a call tab) = the user's own notes as a **timeline of moments**
(`NoteDoc`, no DOM: `ui/my-notes.test.mjs`): each note a line of `my-notes.md` — `- [12:05] …`, `? ` question, `! `
decision, `⚑` flag, `- [ ] [12:22] …` to-do, `  > "…"` the words it is about — kept in time order; a box adds one,
timed from its first key while the call records (a note typed there, or edited in place, without Enter belongs to
its call and is added as Enter would before a close or quit — `prepareClose` —, Write minutes or another call opened;
"Not saved yet" meanwhile; Esc / Cancel drop it); select words in a transcript → Comment, or right-click a transcript turn or a line of the minutes → Add note (the
frame's `EMBED_JS` posts `{echo: 'note', t, quote, section, x, y}`, taken only from that frame) adds one with that quote;
the transcripts show a dot per note at its turn; the Minutes tab shows each timed note as a callout under the
line with the same time (else the nearest within 3 min) — sent to the frame (`{echo: 'notes'}` after its `ready`),
never written into the page or its file; a note is edited in place (click its text: text and kind; its time and
quote stay); writing the minutes again is the ⋯ menu's Write minutes again (no nudge after each note); the process-call skill reads the times, kinds and quotes. Edit as
text = the editor over `my-notes.md` (`NotesSync` in app.js, no DOM: saves 1 s after
typing, on blur, before Write minutes; one acknowledged save in flight per call — IPC `save_my_notes {text, base}` →
`calls::save_my_notes` under `.meta.lock` (`note` takes it too): base unchanged → text; appended since (the tray) →
text + tail; changed otherwise → the other version kept as `my-notes.<HHMMSS>.md`; an unchanged text is never written —
its mtime is the minutes' clock; the answer is a script, `window.echo.saved`). **Minutes** = the page itself,
`/minutes/<call>?v=<minutes revision>` (`minutes::render_for_window`, rendered fresh, under the page's own CSP) in an
`<iframe sandbox="allow-scripts">`; its `EMBED_JS` posts `{echo: 'at'|'open'}` to the window (accepted only from that
frame's `contentWindow`). The window's IPC takes messages only from its own page (`from_page`: wry gives the frame's URL
on macOS — WebKit exposes the handler to frames; Windows listens to the top level only). `render()` replaces each
region (sidebar, list, call header, pane — the Minutes pane's bar and frame apart) only when its HTML changed, so the
frame never reloads for a refresh. Actions go back over IPC (`window::handle` → `{refresh, reply}`: toggle action by id +
status, delete, reveal, record, stop, arm/join meeting, copy, open / reveal the minutes page) through the same functions
as the CLI. The page polls `/api/call/<name>/rev` every 2 s (every file of the call, the live notes', the run's and the
automatic minutes' state) and fetches the call when it changed; a focused notes editor is never re-rendered (a refresh
updates its text in place, caret kept: `syncEditor`). Closing the window, Quit (macOS's own too: tao's delegate gets an
`applicationShouldTerminate:` answering "later", `tray::terminate`), `echoctl app quit`, installing an update and
changing the data folder first ask the page to save (`tray::Leaving`, one at a time: saving — `prepareClose(n, close)`
→ `close_ready {ok, n}`, the editors read-only until Echo is gone or `thaw` — → asking the user — a failure or no
answer in 5 s; a close's failure the page explains itself — → going; `act` does each step). A leave asked meanwhile
replaces the pending one or is dropped (`takes_over`: anything replaces a close, a quit an update / folder change,
`app quit` a quit); while the user is asked it waits for that answer instead (`queued`: a question is answered only
for its own leave), then gets its own turn; once Echo is stopping, every one is dropped. So macOS's waiting quit is
answered by the quit it rides on: yes on exit, no when that quit is cancelled. The question's thread answers once,
also if it fails (a drop guard). `app quit` takes that path with or
without a tray icon (`quit.asking` — this Echo's pid — while the user is asked about it; `quit.cancelled` says why →
exit 1). `app::ask_checked` tells a dialog that couldn't be shown from one not answered (Echo stays, and says so).
Closing the window keeps the app in the menu bar and the Dock (unless Hide from the Dock is on). Minutes need
Claude: a call without them shows when they come, or the `/echo:process-call` command. Dev: `echoctl window
--selftest` (opens the real webview, page fetches via echo:// and reports over IPC; a sandboxed test frame proves
its IPC posts are dropped; the close handshake answers with its number),
`echoctl window --serve <port>` (hidden: the page and its API answered live on 127.0.0.1 by `respond` — a browser against real or generated data, paging and search included; with `ECHOCTL_SERVE_IPC=1` the page's messages too, `POST /ipc` → `handle`, only with that run's SameSite=Strict cookie and its own Origin — for a dev data folder), `echoctl window --export <dir>` (the same responses as files, each minutes page as `minutes/<call>/index.html`, to review the UI
in a browser through a static server).

Calendar (window → Calendar): Day (today) and a rolling Week (yesterday, today, 5 days
ahead) time grid from `/api/calendar?start=&days=` (Graph `calendar_view` for the range,
cached 2 min, served by the async `echo://` handler so Microsoft 365 never blocks the UI;
without a sign-in: today's agenda). Each event carries `recorded` (a call matched by meeting
id, or by subject within the event's time) and `scheduled`; calls matching no event come back
as `calls`. "Record when it starts" → `schedule.rs` (`<home>/.echo/scheduled.json`): the watcher
arms a scheduled meeting from 60 s before its start until its end; the entry is removed when
its recording starts (stopping never re-arms it), when that arm is cancelled, when its recording fails to start
(one notification says so; no re-arm every second), or after the end.
**A meeting nobody recorded, from its Teams transcript** (`teams_transcript.rs`): Calendar → an ended Teams event with
no call → Import Teams transcript (IPC `calendar_import`, in the background — its state on the event, `import`; the
outcome as a notification), ⌘K, `echoctl calendar import <event id | subject words> [--date]`. Graph: the event's
join link → `/me/onlineMeetings?$filter=JoinWebUrl eq '…'` → its transcripts → each as `text/vtt` (`text/plain` is
refused; `OnlineMeetingTranscript.Read.All`, in the CodeMie token). A series shares one online meeting, so only
transcripts started within the occurrence (±30 min) count; several are joined in time order. The call: `meta.json`
`source: teams-transcript` (the event's times — an earlier transcript moves the start, a longer one the end —,
`imported_at`, the AI profile pinned), `meeting.json` from the event (series id, attendees: threads match as for a
recording), `segments.json` + `transcript.md` from the VTT's `<v Name>` cues ("Me" = About me's name, else the
Microsoft 365 account's), the chat (`fetch_call_chat`). **The calendar marks them**: an ended Teams event with no
call whose transcript is known is blue, dashed, tagged "Transcript" (`transcript` on the event, from
`teams_transcript::known`); unknown ones are asked about in the background (`check`, one at a time, ≤ 25 meetings,
the page re-reading while `checking_transcripts`) — about 45 s for 25. `echoctl calendar transcripts [--days]`
(hidden) runs the same check. No audio, no live notes; the list and header say "from Teams
transcript". An existing call of the event (its meeting id, or its subject within its time) refuses the import; no
transcript, or a meeting Teams doesn't give this account (an outside organiser's), says so. Automatic minutes count
from `imported_at` (`notes::auto_due`), so an old meeting gets them. **Needs you**: with the agenda's sync (every 15
min) `teams_transcript::scan` checks the last 3 days' ended Teams meetings of open threads with no call
(`<home>/.echo/teams-transcripts.json`, the calendar's answers too: rechecked hourly until a day after the end, kept 30
days) and offers each
one with a transcript once (Import · Not this → `dismissed`). Measured: a short and a half-hour meeting, each with its cues and chat messages.

## Projects (`projects.rs`; plan and phases: `docs/projects-design.md`)
**The window calls them Threads** (the activity, + Thread, New thread, the palette, Settings — the user's word, since
2026-10-30); the code, the CLI (`echoctl project …`), the API and the data folder (`<home>/projects/`) keep "project".
**Organisations and clients** (2026-11-02, #36): `clients.json` holds organisations (name, other names, domains,
internal, logo, parent, relations, groups); a **client** is one marked `account` (the default: an entry written before
organisations is one, so nothing moved). `projects::clients()` = accounts — Threads, a thread's client, the client page,
the Brain's layers and scopes, senders —; `projects::orgs()` = all — whose people are whose (`people::org_of`, the org
chart, the map), rule matching, Found for you. A thread's **counterparts** are its `orgs` (`OrgPlace`, placed on the map
as before): its `people` rule with no domains of its own takes its client's and theirs; People shows their people
under them. Made from Profile → Organisations (+ Organisation: one you have, or New organisation…; IPC `project {op:
org_link | org_new}`), a person's unknown domain → Name this organisation (`org_name`: new organisation, this
thread's counterpart), Found for you (an unknown domain proposes a new organisation, a counterpart, not a client —
unless New client is picked) and `echoctl org list|add|set|remove` (`--thread` links, `--unlink`, `--account`). A domain
is one organisation's; one with threads under it stays an account; removing one unlinks it everywhere.
**A colleague on any address** (#40, `people::Colleagues`, cached until `clients.json`, the directory or the settings
change): the user's own domain and internal organisations' domains, every address of a directory person with one
of them or known in the tenant (title / department / office from Microsoft 365), and on one invite any address of a
name that also comes with a colleague's address (a colleague invited on a client address). Rule matching
(`CallFacts.colleagues`, `outside`), Found for you, the transcript offers and Needs you, the minutes' colleagues vs
external and People's side all use it; People shows such a person on their own address, the client one in `also`.
Chat messages with no user (a bot or an app) are kept with `bot: true` and are nobody in People; Teams transcripts'
unnamed "@1" speakers are imported as "Speaker 1". Needs you's "Not this" on a transcript offer keeps that meeting out
of the threads it was offered for (`Project.not_events`).
Clients (name, domains, internal) and projects as files: `<home>/projects/clients.json`, `<home>/projects/<id>/`
(`profile.json`, `brief.md`, optional `logo.png|jpg|webp` ≤ 512 KB, checked by its first bytes; a client's logo:
`<home>/projects/clients/<id>.<ext>`; set from Profile — Choose…, or dropped on the tile: IPC `project {op: logo |
client_logo, data: base64}` — or `project set --logo` / `client set --logo`), written atomically
under one lock (`<home>/projects/.lock`). Stages: pursuit (lead, proposal, negotiation), delivery (kickoff, active,
on_hold, wrap_up), closed (done, lost, cancelled); each change is a dated event with its reason. **Membership is
computed, never written into a call**: rules (`series` — calendar series id from Graph `seriesMasterId`, now in
`MeetingContext.series_id`, else the normalised subject; `people` — at least half of the call's **outside** people
(colleagues — the user's own domain, clients marked internal — don't count) from the domains given, else the client's; `chat` — `<call>/chat.id` or the join link's chat; `subject` — whole words), pins
and exclusions (call stamps), `since`, and a closed project takes no call started after it closed. Suggestions are
deterministic (half the call's people are the project's, or its subject names it or a glossary term), never an
assignment; an exclusion ("Not this") stops them. **Found for you** (`projects::found`, `#found`, `echoctl project
found [--accept KEY --name --client new|ID|""] [--merge KEY --into ID] [--reject KEY]`): calls of the last 90 days in
no project, two or more sharing a meeting series, one outside organisation (the domain half their outside people
come from; personal mail and colleagues aside) or one Teams chat → a candidate with its rules, calls and client (one
you have, or a new one named from the domain); rejected keys in `<home>/projects/found.json`; its count badges the
Projects activity. `echoctl project …` / `client …` (JSON; refused inside a run Echo
started). The minutes: `context.projects` = each project's brief, notes for Claude, glossary, people and the
highlights of its last 5 calls before this one (`calls::recap_from_notes`), so the guard stays as it is. Transcription:
`vocab::for_call` = always-on + the glossaries of the projects the meeting belongs to + learned; `vocab add` from a
project's call adds the terms to its glossary too. The window: **Projects** activity (`#project/<id>/<tab>`; list by
client, Closed folded; each client an accordion — its chevron folds it, the open thread stays in view, a count shows; remembered in the webview, `echo.threads.folded`; tabs Brief (rendered `minutes::markdown_html`, edited in place), Calls (why each belongs,
Unpin / Take out), Profile (stage bar, client — + New client —, since, look, rules, glossary, people, notes, AI
profile + model)); a call's header shows its projects as chips, a suggestion bar (Add · Not this) and `+ Project`
(several; New project from this call = that call pinned + its meeting series as the first rule); the calls list shows
their icons; ⌘K goes to any project. A project is drawn as its logo, else its client's logo when picked (`icon: "client"`), else a symbol picked
from `PJ_SYMBOLS`, else an identicon from its id (`ProjectArt`, no DOM: `ui/projects.test.mjs`), in its colour. API: `/api/projects`
(overview + `by_call`), `/api/project/<id>[/logo]`; IPC `project {op, …}` (`window::project_op`).

**The work ledger** (`ledger.rs`, taska's ported; `<home>/projects/<id>/ledger/`): `events.jsonl` (append-only, one write
per batch) folded into Workstream (epic, `E01 General` first) › Story (criteria) › Task (action / session /
dependency, owners, due, client-owed) + decision / risk / question records. Stories and tasks carry a `description`
(Markdown; an agent's from `create` / `revise`, ≤ 1200 chars; the user's in Details — click it — ≤ 4000, pinned) and
`links` (`{title, url}`, https, ≤ 10: tickets, docs). A record's substance lives in the same `description`, named by
its kind (`RECORD_TEXT`): a question's **Answer** (answered = an answer, or `done`; `ledger::answered` / `record_open`), a
decision's **Decision** (what, the options, why — an agent's at `create`) + `decided_by`, a risk's **Response** + owner
(`internal_owner`), `likelihood` / `impact` (low | med | high; with a response it reads "handled", without "no response").
An agent's answer or response is op `answer` (`text`; a risk's owner and levels with it), verified like a status change
(`supports` → set; a question closes) — never a `description` on a question or risk; the user's edit pins it. `context`
shows each record with what it holds; the client Overview lists open risks without a response first, marked; the
`<echo_view>` item carries it. **Mails as evidence** (`echoctl project ledger <id> mail
<graph message id>… [--conversation]`, `ledger::add_mail`): one item a mail, `mail-<sha8 of its Internet message
id>` (kind `mail`: one turn — the sender ("Me" for the user's own: About me's address or the account's local part; names
through `people::display`), the local time, the new part of its text — and its Outlook `webLink`),
read by the next sync like a call; the user's pick is the trust. A thread's ✉ Mail sources (the Brain's `domain:` /
`person:` rules) are read into its work in each sync too (`ledger::mail_sync`, `sync --mail`; first 30 days, later 7,
≤ 200 a read; state in `ledger/mails.json`, on the source's chip; trusting a sender — or reading one's unauthenticated mail anyway — makes
that client's threads read their mail and meeting chats from the start again, `ledger::reread_sources`): only the user's, colleagues' and senders trusted for
the thread's client — a trusted outside sender's only with DMARC/compauth passing, the Brain's `senders` rules; others
wait there (Needs you — asked there also with the Brain off when a thread reads mail or chats); no organisation domains
known → nothing read. Read whether or not the Brain is on (not
without the `brain` feature). History entries from a mail show ✉ and open it in Outlook. Evidence = each project call as an item
(`evidence/<stamp>.json` + `.md`: transcript turns `[hh:mm:ss] speaker: text`, chat and the user's notes as text; a
changed call is queued again by a `reprocess` event). `apply` answers with what it `created` (ids, titles, parents) and `touched`, so the next item and the dependency pass refer to
them; a new story or task whose title shares most words with an open one of the thread is offered "Same as …?" (To review, a
`suggest_merge`, never merged by itself; #39). Each proposal's quote must be one contiguous span of
its item (normalised), level rules, nothing before it existed, re-applying adds nothing; status changes, criteria
met, links and waits count only after `verify_apply` says `supports`; the user's edits (`human_event`) pin fields
(an agent's later evidence stays in the history, `applied: false`) and can be undone; spoken dates resolved against
the item's day (`resolve_due`). **The sync** (`projectsync.rs`): `ledger::sync` (the first one: the last 20 calls,
`BACKFILL`), then two Claude Code runs of the plugin's `project-sync` skill (EXTRACT / DEPS / VERIFY.md) — extract
(+ dependencies), then verify in a fresh process — on the project's AI profile and model, guarded (`guard.rs`
`RUN_MODE=project`: read the ledger, the skill's folder and the session's own tool-results — a large thread's `context`
or `people-context` Claude Code saved to a file (#39) —, write only `.json` in `runs/<run>/`, Bash only
`echoctl project ledger <this id> <PROJECT_STEPS>`; the CLI checks `ECHOCTL_RUN_PROJECT` too and keeps `verify-prep
--out` in the run's folder); ≤ 10 items a run; state in `ledger/sync.json`, the log in `runs/<run>/log.txt`. The app's
scheduler (every 5 min, `config.project_sync`, default on; Settings → AI & minutes) syncs open projects with a call
ended ≥ 15 min ago not read yet; a failed sync waits an hour. `echoctl project ledger <id> run|status|tree|edit …`;
the window's **Work** tab (a **board** as GitHub Projects lays one out — fixed-width columns that scroll sideways, each a
project's own Status option in a state category (`Project.board`: columns with name, colour, `state` open | in_progress
| waiting | blocked | done, limit, hidden; card fields; group none | story | owner), configured in each column's ⋯ and
`+ Column`; saved **views** as tabs (`board.views`: layout board | tree, kind / owner / open-only filters, grouping,
card fields, hidden columns; the one in use remembered per project in the webview), cards dragged between columns set the item's state and `column` (a human field, pinned) —, or a tree by workstream, a node's fields, criteria, history with quotes and verdicts, Undo of
your edits, To review — suggested moves/merges, weak "done" claims —, Sync now; `/api/project/<id>/work`, IPC
`project {op: sync | ledger, body}`).
**The meeting chat between calls** (`ledger::chat_sync`, a thread's `teams_chat` sources — Profile → Sources offers its
calls' chats, `+ 💬 <meeting> chat`; `echoctl project source <id> --chat 19:… --name`; IPC `source_add`; read whether or
not the Brain is on): in each sync (`projectsync::run`; `due` also when a chat wasn't checked for 30 min or a chat day
isn't processed) the chat is read through Graph (`graph::chat_history`, ≤ 2000 newest; the first read 30 days back,
later 7 — days are rebuilt, an item that changed is processed again, `reprocess`), messages within ±2 min of this
thread's calls on that chat left out (the call's item has them), only the user's, colleagues' and senders trusted for
the thread's client (the Brain's `senders` list; others wait there; an author not looked up yet holds the day), one
evidence item a day `chat-<sha8 of chat>-<day>` (kind `chat`, turns at local HH:MM:SS, files shared named), written
once the chat has been quiet for `SETTLE`. State per chat in `ledger/chats.json` (read_at, days, found, error), shown
on the source's chip. `project ledger <id> sync --chats` reads them by hand. Measured on one thread:
days of messages between its calls held a blocker said nowhere in a call.
**A client's page** (`#client/<id>/<tab>`, `/api/client/<id>` = `tasks::client`): Overview — its description (the
user's Markdown, `projects/clients/<id>.md`, edited in place like a thread's Brief: IPC `project {op: client_brief}`) and
a card per thread (stage, calls, stories done, open tasks by pile with the late ones, the last 3 decisions, open risks
and questions; a record opens in its thread's Work tab) —, Calls (every call of its threads, with their chips), Tasks
(the Tasks piles for its threads), Profile (name, other names, domains, kind, logo, threads).
**People** (`people.rs`; plan and discovery: `docs/projects-design.md`, People): a People tab on every thread and client
(`/api/project|client/<id>/people`, `echoctl project|client people <id>`). Facts are recomputed from the calls each time
(invitees, organisers, named speakers and their share of the talking, chat posters; calls attended, first and last
seen → **new** ≤ 14 days, **quiet** > 6 weeks while calls went on; open tasks owed, late, waiting on them by name); names are read as people read them (`people::display`: "(Partner)", "(external)", "[EXT]" dropped, a directory's
"Last, First" turned round; compared with accents folded); one person can have addresses at several organisations —
the same cleaned full name merges by itself (the other address kept in `also`), names or address local parts that
share a first and a last name are suggested ("Same person as …?", `maybe_same`), and the panel merges any two
(`person_merge`: the directory keeps every address of a person, `canon`) or splits an address off (`person_split` →
`not_same`: never merged again); each address shows its organisation; the user is left out. What the user sets: the directory `projects/people.json` (title, organisation, group, reports to,
matters, phone, notes, name — each with its source and a history; IPC `person_set`), placements in a thread
(`Member`: role, stance champion…blocker, influence and interest 1–3; `people_place`, `people_add`) and organisations
as stakeholders (`OrgPlace`, `org_place`); an organisation (a client) has a parent, typed relationships (`RELATIONS`)
and groups (`client_set parent|relation|groups`). Views: **Map** (influence × interest quadrants, high = 2+, chips
dragged between them set both; Not placed yet; organisations as square chips), **Org chart** (#43: who does what in this engagement, not reporting lines — rarely known. #46: the layout is `OrgLayout` in app.js, no DOM,
`ui/org-chart.test.mjs` — a "<X> stream lead" leads column X and stays out of the band, the band's high influence and
interest fallback is for people with no stream; Keep these columns saves the columns read from roles as each person's
`group`; the "works for" hint only within one stream and never from governance, PMs or partners (`people::coordinates`,
`role_stream`); "new" = first seen more than a day after the thread's first call; a role keeps `role_by` (you: pinned —
never proposed over — / echo), `role_at`, `role_from`, and the people pass may propose a replacement ("was …"); the
pass proposes `person` — someone named but never invited, one proposal per name with its mentions counted —, on Yes a
name-only person in the thread under the organisation the words gave. Each organisation, yours on
the left and the others mirrored right, has a governance band — `Member.governance`, else high influence and interest —
then a column per workstream — `Member.group` in the thread (the people pass's `group`), else the directory's group,
else read from the roles while none is set ("<X> stream lead", a shared "<X>:" prefix; shown "from roles", never
saved) —, its lead on top (whom the others work for here, or a "lead" role), the rest by title seniority
(`people::seniority`, ordering only), inactive people folded "+N"; one card: role, stance, open / late tasks, last seen.
Lines: works for here (`Member.works_for`, solid; the work's suggestion `works_for_hint` — who handed their tasks over,
else their stories' owner — dashed), works with across organisations (shared calls, faint), reports to only when asked
(dotted) and in the card's tooltip and the panel. In a thread a card dragged onto a person sets works for, onto a
column its workstream, onto the band governance; + Workstream adds a column; organisations' relationships drawn as
lines between the cards after layout, `drawOrgLinks`: labelled with their kind, "part of" dashed, card edge to card
edge, a related organisation with nobody in the thread shown as an empty card so the line has an end), **List**
(contacts with Teams chat and copy email; Seen in calls → Add / Add all); the person or organisation in the right
panel (`person`).
**Microsoft 365** (`graph::person`, `people::refresh_m365`): the People API (`/me/people?$search="<email>"`,
`People.Read`) gives colleagues' title, department, company, office and phone; people outside the tenant come back
with a name or nothing. Measured with the CodeMie app (2026-10-30): a token carries every scope the tenant
consented for the app whatever a sign-in asks (People.Read, Group.Read.All, Mail.Read… — no new sign-in needed), but
not `User.Read(Basic).All`: `/users/{x}` and their managers are refused, so reporting lines come from calls and the
user. Asked when a People tab opens (in the background, one refresh at a time, each person at most weekly, ≤ 40 a
run; `echoctl project people <id> --refresh`); a value goes in with source `m365` unless the user set that field —
then it becomes a question in the person's panel (Use theirs / Keep mine: `person_answer`; a kept value isn't asked
again).
**The people pass** (the sync's third run, after extract and verify; `PEOPLE.md` in the project-sync skill): reads the
calls it hasn't read yet (`ledger/people-read-2.json` — version 2 re-reads everything once for facts; ≤ 10 a run, also when the
work has nothing new) with the thread's people in view (`project ledger <id> people-context --items …`) and proposes
role, stance, influence, interest, title, group, reports to, left, and relationships between organisations
(`people-apply --evidence --file`, both in `guard::PROJECT_STEPS`). `people::apply_suggestions` checks each: the quote
one span of that call (`ledger::quote_in`), the person or organisation known, the value valid and not already so, not
proposed or turned down before → `<project>/people-suggestions.json`, pending. Nothing changes until the user answers
in the People tab's bar (the question, the words, the call and time; Yes applies it as theirs, No stays no, Change…
applies the user's own version — checked as a proposal is, state `corrected`, the proposed value stays turned down; Needs
you's Change… opens that editor in the People tab: `people_decide {value}` → `people::decide`). No model verifier on top: the quote check and the user's yes are the gate. The same pass keeps each person's file (as a vault's people
notes): **facts** (`fact`: one line ≤ 240 chars, its quote one span of the item → `Person.facts` in the directory at once,
`{text, quote, date, thread, evidence, kind, call, url, ts}`, deduped by person + item + quote, ≤ 300; a mail's or chat's
sender may be named by address) and **who they are** (`about`, ≤ 800 chars, no quote, source `echo` +
`about_thread`; the user's own — Details, ≤ 2000 — is pinned and never rewritten). The view adds `about`, the facts of
its threads, `threads` (by calls, places, facts), `touches` (distinct sources) and `with` (≤ 3 people sharing ≥ 2
calls; built again only when its inputs' file times change, or every 10 min — `view_cached`). **The Brain's People**
(`#brain/people`, `/api/brain/people` = `people::everyone`): threads on a ring (a
client's side by side), people near theirs — only those with ≥ 2 sources, a place or the user's description (a one-off
name stays out); role lines solid, calls faint, reports-to dashed, calls-together on selection; a click opens the
person panel (Who they are, What was said about them with links to the call / mail, Often in calls with). MCP
`find_people` / `get_person` (echo-brain): only the threads on the reader's profile, narrowed like its documents
(`Access::people_threads`); Echo's `about` only when written by one of them.
**Tasks** activity (`tasks.rs`, `#tasks`, `/api/tasks`, `echoctl project tasks`): every open project's open tasks in piles
by the user's rules (2026-11-02) — **I owe** (owner = the transcripts' "Me", or `config.me`'s name, first name or email;
a blocked one stays here, marked "blocked by …"), **Owed to me** (someone else does it and the user handed it over: the
task's `delegated_by` is me — a human and agent ledger field, Details → Handed over by, EXTRACT's `delegated_by` on
create / reassign — else the speaker whose words created it; tasks filed by hand count only with the field), **Waiting
for others** (what stops what I owe: each open task my tasks wait on that isn't mine — "stops T011" —, and a card per
person a task of mine waits for, `waiting_for {who, what}`), Nobody named, and **Others** (other people's work I neither
handed over nor wait on; folded under the columns) — overdue first, then by date, then the longest quiet (days since
the last word in the ledger); a project filter and an order. **Asks** (right panel): by person, what they owe me (Owed to me) that is late, due within 3 days or quiet ≥ 5 days, and
what my work waits on them for (an owed task that also stops me asked once); ≤ 2 per story, ≤ 15 in all; each person's next meeting in the next 7 days (the calendar's cache,
attendees by email or name; no sign-in: today's agenda). A card or an ask opens the task's details (the Work tab's
`work-item` panel) right there.
**One piece of work in two threads** (#20): a work item linked to one of another thread — `same` (one piece of work, its
master where it is done, `master: here | there`) or `depends_on` (the other side records `blocks`) — as the user's edit in
**both** ledgers (`xref` / `unxref`, `ledger::link_threads` / `unlink_threads`; an undo of either side unlinks both; both
items must be open). Read through `ledger::fold_linked` (the window's tree, Tasks, the extractor's context): a master takes
the verified status changes its mirrors' threads heard (newer than its own; history marked `from`), a mirror shows its
master's state, owners, due and last history (`mirror_of`) while its own history keeps its evidence, `also_in` names the
mirrors, `xwaits` the open items it waits on elsewhere. Tasks shows a pair once (the master's card with every thread's
chip) and a cross-thread wait in Waiting for others. Details: Same as / Also in / Waits for with Unlink, and **Link to an
item in another thread…** — `/api/work/search` (`ledger::search_open`: open items by title words, no model) → Same work ·
done there / Done here / Waits on it. EXTRACT sees mirrors as known (`also_tracked_in`), so it doesn't create them again.
**What this call changed** (a call's Actions tab, above its action items; `ledger::call_changes`, call detail `changes`):
per thread the call is in, every ledger history entry whose evidence is this call (new stories, tasks, decisions,
risks, questions; status, dates, links, waits, criteria met; mentions left out, ones the verifier called wrong
hidden), in the order said, with the words, the speaker, the time (a jump) and weak / to check / kept yours; a
thread that hasn't read the call says so (Sync now). An action item that became a task (`ledger::match_actions`: a
task created from this call within a minute of the item's moment sharing a third of its words, or half with no
moment; ¾ when farther) shows its task as a chip instead of a second line (`action_tasks`).

## The Brain (`brain/`; plan and phases: `docs/brain-design.md`)
Cross-call intelligence as Echo's own: Brain (onetest-ai/applied_skills `bundles/brain`) ported into the app — **one per
user**, **facts only** (calls now; Teams channels, mail and thread folders next — the stakeholder map and the work ledger
are referenced by id, never ingested), **nothing copied** (every section links back: `echo://call/<name>?t=<s>`, later a
message's / mail's / file's own link), **no model in Echo** (embeddings from the endpoint in Settings → Brain: Ollama
`/api/embed` or OpenAI-compatible `/v1/embeddings`, `<config dir>/brain.json`, 0600; without one, words only). Sources are
configured where they live (a thread's Profile), Brain settings in Settings → Brain, exploring it in its own activity
(phase 3). The `brain` cargo feature (default) brings SQLite (`rusqlite`, bundled — C, so the Windows cross-check runs
without it).
- **Store** `<home>/brain/knowledge.sqlite`, Brain's schema (`chunks` + Echo's `link`, `refs`; `chunks_fts` FTS5
  `unicode61` — any script, Brain's own query builder dropped Cyrillic; `chunks_vec` a plain table of unit vectors
  searched in Rust, no sqlite-vec; `documents`, `related`, the taxonomy tables, `meta`). Brain's chunker and chunk ids
  (`sha256(source\0ord)[:8]>>1`) as they are (`chunk.rs`, 1200 chars, no overlap).
- **A call** (`calls.rs`, source `call/<stamp>`): its minutes' sections, then the transcript as packs of turns
  (`### [mm:ss] A, B`, lines `[mm:ss] Name (Org): text`, ~1000 chars, real names via `people::display`, organisation
  from the invite's domain), then the chat; refs = the call, its threads and client.
- **Teams channels and mail** (`sources.rs`, phase 4; a thread's Profile → Sources: # Channel — a team, then a
  channel — and ✉ Mail — a domain or an address, the client's domains offered —, `echoctl project source <id> --mail
  | --channel <team>|<channel> --name | --remove kind:id`, IPC `project {op: source_add|source_remove}`, pickers
  `/api/m365/teams|channels`): read with the Microsoft 365 sign-in (`ChannelMessage.Read.All`, `Mail.Read`, already
  consented for the app) when the calls are synced and every half hour, first 90 days, ≤ 200 a read; one document per
  channel post with its replies (`channel/<key>/<id>`, titled by its first line that isn't a bare link) and per mail
  conversation (`mail/<conv>`, each message's new part, a section each), every section linking to the post / mail in
  Teams / Outlook — nothing kept but the index. Mail: Graph's `from:X OR to:X` (its search is fuzzy, `participants:`
  fails) then checked by address; calendar invites (`eventMessage…`, the call is the meeting), mail to > 30 people
  and the user's own addresses at any domain (by the account's local part) don't count. Channels: system events
  dropped. A document's threads = every open thread whose sources match it, recomputed at each sync (`rescope`: refs
  and scopes rewritten, nothing fetched; matching none → removed). Read state per source in `<home>/brain/sources.json`
  (`since`, `found`, the error shown on the chip); its `reader` version, when bumped, drops and re-reads everything.
  The Brain's Sources tab folds a channel into one row; the graph draws posts and mail square.
  **Who may speak, and only as facts** (`senders.rs`, `digest.rs`; reader 4): a message is read only from the user,
  colleagues (mail sent inside the organisation — Exchange's `AuthAs: Internal` — from the tenant's verified domains,
  the user's own address too; a Teams author whose People-API entry has the author's own directory id, never a name
  match, and an address in them) or a sender the user trusted for every client of the document's threads
  (`<home>/brain/senders.json`, per client, `-` = none; ignored anywhere wins); a trusted outside sender's mail only
  when Exchange's own (topmost) `Authentication-Results` says DMARC or compauth passed for the From domain itself —
  SPF + DKIM alone can pass for an attacker's domain (`graph::mail_auth`). An author not looked up yet (≤ 40 lookups a
  sync, or it failed), or no organisation domains known, defers the document — never a stranger's verdict. Others wait (count, last subject; nothing of
  theirs read) for Trust / Ignore / Forget (the Brain's Sources tab, IPC `brain_sender`, `echoctl brain senders
  --client C --trust|--ignore|--forget KEY`). A trusted sender's mail with no verdict at all (a mailing list, a
  domain without DMARC) is listed (`List.unverified`, Needs you, Sources) for Read anyway / Keep out
  (`Entry.unverified`; `read_unverified` | `keep_unverified_out`); a failed check is never read. A decision asks for that client's sources to be read again
  (`brain/reread.json`, taken by the next sync: their documents dropped, read from the start; the cache keeps it
  cheap); a sync saves senders merged (`save_merged`: the user's lists on disk win).
  Then the **quarantine reader**: each post (with its replies) or conversation once, by a no-tools model on its
  threads' profile (`livenotes::complete_text`), into a neutral title and facts per message, each with at most a
  25-word quote checked word for word against that message (else dropped); cached by content, profile and
  `PROMPT_VERSION` in `<home>/brain/digests/`. The Brain indexes those facts with links — never the messages' text,
  so a forged instruction in a mail never reaches a session with tools. ≤ 30 read a sync; what waits keeps its
  sources from reading further; a document the reader fails on 3 times is given up; the reader waits while a call
  records or is transcribed (`insights::quiet`). Measured: 36 posts + 1 conversation in ~5 min (two syncs) on haiku.
- **Thread folders** (`brain/files/`, phase 5; Brain's `parse_corpus` + `visual-parse` + `source_registry` +
  `scribe_marker` ported; plan and measurements: `docs/brain-design.md`). A thread's Profile → Sources → ▣ Folder (the
  system's picker, IPC `source_folder_pick`; `echoctl project source <id> --folder <path> [--exclude glob…]`), "Don't
  read…" globs per folder (`Source.exclude`, IPC `source_exclude`). **Registry** (`files` in the store; `folder.rs`):
  root + relative path, sha256 (hashed only when size or time changed), status + reason — `indexed`, `pending` (pages
  waiting for the page reader), `skipped` (hidden: dot components, `~$` lock files; excluded by a glob; too big;
  Echo's own minutes page by its generator tag), `twin` (same stem in another format: pptx > docx > xlsx > … > pdf is
  read), `unsupported`, `error` (the reader's words); the same bytes at a new path are a move (its document keeps its
  id `file/<id>`); gone → removed with its sections; `READER_VERSION` bump re-reads all. **Readers** (deterministic):
  pptx/docx from their XML (`office.rs`, `xml.rs`: text in order, headings by style name, table cells as Markdown
  grids, speaker notes, SmartArt text; hidden slides out), PDF through PDFium (`pdf.rs`, BSD, loaded at runtime:
  `ECHOCTL_PDFIUM_DIR` → `Echo.app/Contents/Frameworks/pdfium` → `<config dir>/pdfium`; `scripts/fetch-pdfium.sh`
  pins chromium/7881 by SHA-256), spreadsheets (`sheet.rs`, calamine: the densest early row as header, a section per
  row; a sheet over 200 rows makes it a reporting book — schema + 8 rows per sheet, its figures for the numbers lane
  later), vtt/srt packed turns, md/txt as is, html as text (degraded; < 220 chars = JS-rendered, skipped). **Routing**
  (`route`, Brain's `render_pages` rule on PDF pages): thin text (< 220) with no table, ≥ 60 vector paths, or ≥ 28 with
  < 1000 chars → the page reader. Decks and Word files with figures are measured and rendered on LibreOffice's PDF of
  them (`render.rs`, `soffice --convert-to pdf`, own profile, once per file version in the assets; slide N = page N,
  checked) — the slides' own XML misjudged half of the slides in a sample of real-world decks (free text boxes, divider lines); their text and
  tables still come from the XML. No LibreOffice → slides indexed from their text (`degraded`). **Page reader**
  (`vision.rs`): Claude on the default profile's minutes model (`livenotes::complete_vision`: images as content blocks
  over stream-json input, no tools) or a vision endpoint (Settings → Brain → Documents: Ollama `/api/chat` /
  OpenAI-compatible), 4 pages a request, Brain's deck rules (flow → ordered list, timeline → table, diagram →
  relations, extracted cells trusted for numbers, no logos / page numbers / confidentiality marks), the answer checked
  (one entry per image); cached in `page_render` by image sha + `PROMPT_VERSION`; ≤ 48 pages a run, only while quiet
  (`insights::quiet`), the rest `pending` (indexed from their text meanwhile; the scheduler re-checks every 10 min).
  **Two representations**: the reading (or text + tables) is the document (`## pNN · title` per page, links
  `echo://file/<id>?page=N` — the window opens the registered file in its app, IPC `open_brain_file`); the page's text
  layer, table grids and PNG (flagged pages) stay in `<home>/brain/assets/<id>/` and are what MCP `get_evidence`
  returns (`verbatim_page_text`, `extracted_tables`, `page_asset`). **Dates**: Brain's `exact_date` (the first full date
  in a document's first 2000 chars — not ≥ today: an auto-updated field; else a full date in the file name;
  transcripts and spreadsheets by file name only). The Brain's Sources tab folds a folder into one row listing every
  file with what happened to it; `echoctl brain files`; `echoctl brain read-file <path>…` (hidden) shows a reader's
  pages and routing.
- **Sync** (`store::sync`): a document re-chunked only when its text or refs changed (hash), a chunk re-embedded only
  when its text did, chunks gone deleted with their FTS row, vector, related edges and tags; a snapshot (`VACUUM INTO`,
  3 kept) before a change, one transaction; vectors outside it in batches of 32 (an embedding failure leaves the text
  indexed; the next sync fills the gaps); another model re-embeds all; `related` = Brain's (k 6, ≥ 0.55, other
  documents). The app syncs every 5 min when the calls' files, the threads or the model changed (`start_scheduler`,
  switch in Settings → Brain); Sync now there (IPC `brain_sync`).
- **Search**: BM25 + the query's vector fused by Brain's weighted RRF (0.4 / 0.6, k 60, 30 each).
- **At scale** (`browse.rs`; measured on a copy grown to 50,000 sources, 2 million sections, 20,000 terms): the window
  never loads all of it. `documents` keeps its list facts (`sections`, `client`, `root`; `store::DOC_FACTS`, filled once
  for an older store) and `chunk_topics` is indexed. `/api/brain` carries no documents and no terms (`terms_n`; a
  thread's 30 for its Profile, `terms_more`); `/api/brain/browse/…`: `groups` (per client its kinds, channels, folders),
  `sources` (50 a page, newest first, cursor `date\tsource`; `of`, `kind`, `channel`, `q`), `folder` (its registry's
  files, 50 a page), `terms` (group counts, a group A–Z from a `letter`, 50 a page, or `q` across groups), `graph`
  (level-1 topics + their related pairs, ≤ 6 each), `graph-topic` (its 50 most tagged sources), `graph-client` (≤ 400
  sources, each joined to its 3 main topics), `graph-search`. Aggregates over every section (the taxonomy view's
  counts, drift, coverage, scopes, the groups, the topic graph) are `store::kept`: kept while the store is unchanged
  (`changed_stamp`: the file, and the write-ahead log only while it holds writes), recounted in the background when it
  changed — the last answer meanwhile, the fresh one if it comes within 250 ms; the app counts them once at start. The
  window: Sources and Terms as closed accordions opened a page at a time ("Show 50 more · N left", the pane keeps its
  scroll); the Graph starts with topics only and grows in place (`GraphView.batch`: changed with the renderer off —
  sigma answering each dropped node and edge one by one froze the page), a topic's click, a client's chip or zooming
  in on its topics, a search's Enter each bringing a set, ≤ 400 sources on screen (the oldest set goes first). Term
  mentions are found once per section (`term_scan`: the text it was read in; a sync reads only changed sections,
  `mentions_changed`), each name indexed by its rarest word; an edit re-reads only the terms it changed through the
  full-text index (`mentions_of`) — measured: 21 s the first time on 2 million sections, then ~2.5 s an edit, which
  runs off the window's thread (`topic_jobs` `edit:taxonomy`, its error once as `edit_error`). Near-duplicate terms
  (`review_flags`) are compared with their A–Z neighbours, never each with each.
- **MCP** (`mcp.rs`, `echoctl brain mcp`, stdio JSON-RPC, read-only): Brain's tool names and shapes (health,
  search_knowledge, list_sources, read_document, find_related_content, get_evidence; get_taxonomy / list_metrics /
  get_metric answer `not_modeled` until those lanes exist) — kb recognises a Brain by them. Attached to both window
  sessions (`--mcp-config`, `mcp__echo-brain` allowed); the call session's guard allows `mcp__echo-brain__*` in ask mode
  only (`guard::BRAIN_TOOLS`), never in a minutes run.
- **It tells Claude it is there** (#34, `brain/advert.rs`, no model, nothing when it is off): what this reader's Brain
  holds now — kinds with counts and dates, clients and threads with their ids, people / figures / insights, last update —
  from the store and the reader's access, plus fixed rules on when to go there first (what was said / decided, people,
  figures, before a brief or prep; scope by client / thread; "not in the Brain"). In the MCP server's `instructions`, an
  `<echo_brain>` block at `SessionStart` of Echo's sessions (narrowed like the session's server; again after compact /
  clear, on resume only when changed), routines' prompts (`claudejob`, when they have tools) and a `Brain:` line in
  `<echo_view>` for an open call, work item, calendar event, client or thread. Usage counted (`brain/usage.json`: tool
  calls by day and session kind — `brain mcp --who folder|call|routine` —, Also discussed / Mentioned also in views),
  shown in Settings → Brain. Measured on a working store: asked about a client, a fresh session loaded the tool and
  searched scoped to that client first.
- **Client and thread are its dimensions** (`scopes(chunk_id, kind, id)`, from each section's refs; `-` = a call in
  no thread / with no client; filled once for an older store): search, related, the taxonomy's counts and the MCP tools
  take `client` / `thread` (`list_scopes` names them, the server's instructions say to scope); moving a call to another
  thread moves its sections (refs are part of a document's hash). There is no Ask screen: asking is a Claude Code
  session with the Brain attached.
- **Two layers**: a shared layer (the user's practice: calls with no client) and one per client (its goal from the
  client's name, description and threads; its own open term kinds); node ids of a client's layer carry its id
  (`acme__budget`), so the same name in two clients is two topics; a section is tagged from the shared layer and its
  own client's only; a client's terms are found only in its sections; a scope's view shows the shared layer and its
  client's; an edit names its layer when a name is in two. A version from before layers loads as the shared layer.
  **One tree** (2026-11-02): layers are how topics are *built and owned*, not separate trees. A client's topic may
  sit under one of the practice's level-1 topics (`Topic.parent` names it; `taxonomy::parent_of`: its own layer's
  first, else the practice's); building a client's layer gives the model the practice's level-1 topics to place its
  own under, and this layer's existing topics to keep (`keep_identity`: a new wording of an old topic — its label,
  a synonym, or ratio ≥ 0.86 — keeps the old label, id and tags, the wording becomes a synonym). Topics have
  **synonyms** (`Topic.synonyms`: old labels from renames, merges and rebuilds, the user's; tagging resolves them);
  a description is the definition tagging reads (Define… in the panel). Tagging reads one tree (`vocab_text`: the
  practice's topics with the clients' under them, "(Client)", then "[Client only]"). A client's topic never tags
  another client's section (`build_graph` drops such tags). **Moves** (`taxonomy::moves`, no model; Inbox, the
  Topics tab, `echoctl brain taxonomy moves [--apply|--dismiss KEY]`): up — clients' topics naming one thing (a
  label or synonym in common, or nearly the same label) → the practice (`scope` op, the others merged in as
  synonyms; `merge` takes `into_layer: shared`); down — a practice topic whose ≥ 10 tagged sections are all one
  client's → that client. Turned down ones stay quiet (`brain/taxonomy/moves.json`).
  `taxonomy build [--layer shared|<client>]` builds every layer or one (then classifies that layer's sections).
  **A client never repeats the practice** (#32, #33, `fold_into_practice`, after every client build): a client topic that
  names a practice topic (label, synonym, ratio ≥ 0.86) isn't made — its subtopics go under the practice's level-1, its
  tags follow by alias — and a client term that is a practice term by any name isn't either (its thread pins move to the
  practice's). Duplicates from older builds are offered as Move up ("your practice has it already"). Move up says where it
  went (a term: the dictionary, its group). **Split** (#31, `taxonomy::split`, `echoctl brain taxonomy split <layer>
  <topic>`, IPC `brain_split_topic`, the drift banner's and the Inbox's first button when one topic took the most, the
  topic panel): that topic's sections only (≤ 80 sampled) → 2–6 subtopics (a level-2 topic is replaced by siblings) →
  provisional → only those sections tagged again (`classify_in`). A rebuild gets the drift reasons and each topic's
  sections and share, and is told a topic over a quarter of the sections gets subtopics; the catch-all reason names its
  topic.
  **Drift** (#47, `taxonomy::drift`, no model): per layer, over the sections that came in since its last build (its `build`
  in the history: indexed after its `at` — `chunks.created_at` — and judged since; never the build's own sections tagged
  again, #18) — ≥ 30 of them —, stale when > 10% fit no topic (`untagged`) or one level-1 topic took > 40% of the tagged
  ones (one with subtopics: only when one subtopic did) — these two only for a layer built from ≥ 6 calls (`built_from`
  0, unknown, is judged); or when its calls are ≥ 3× `built_from` (and ≥ 6). A stale layer is an Inbox item ("Topics for X need a rebuild", the reasons in plain words — `reasons[].text`, one source for the Inbox and Builds) and a banner on its Topics
  block: Rebuild topics (IPC `brain_rebuild_topics` → `induce` in the background, provisional as any build; a
  notification when done) · Not now (`brain_topics_snooze` → `brain/taxonomy/drift.json`: held until its sections or
  calls grow by half; a rebuild clears it). Never rebuilt automatically. `echoctl brain taxonomy drift` (JSON).
- **Insights** (`insights.rs`, `<home>/brain/insights.json`): per client and thread — moved, slipping, recurs,
  contradiction, open — each citing ≥ 2 sections from ≥ 2 sources, every quote checked word for word (else dropped). A
  scope is redone only when its sections changed (a fingerprint of ids and texts): the run reads the previous
  insights, the new sections (≤ 120, newest first) and their related older ones, and keeps / updates / retires / adds.
  The app's scheduler runs scopes at most daily each, ≤ 5 a day, only while nothing records and (macOS) on AC power;
  Refresh (IPC `brain_insights`) or `echoctl brain insights --client|--thread [--run]` runs one now. A cited section
  changed or gone marks the insight "to re-check" (no model). Shown on a thread's Brief and a client's Overview.
- **Numbers** (`brain/numbers/`; Brain's `tabular-semantic-layer` ported — the model picks a metric and filters, SQLite
  gives the value from the real cells with its file; a number is never asserted by a model). Per client, from its
  threads' folders' spreadsheets; config in Brain's own format, `<home>/brain/numbers/<client>/families.json` (families:
  glob, month_from, layout `long` | `tolerant_long` | `matrix_month_cols` | `wide_month`, grains, measures with
  alternatives, header / banner rows, collision policy, entity_map / entity_with, allow_zero; conform_titlecase,
  dimension_map, rollups, derived, coverage) + `metrics.json` (catalog: unit, desc, definition — reported vs computed).
  **Profile** (`profile.rs`, no model): each report series (same folder and name but dates) once in full — sheets,
  header row, columns, samples, first-column labels — and its other files' sheet headers (template drift). **Propose**
  (Claude, default profile's minutes model): the config + catalog + what it skipped and why; then **repair rounds**
  (≤ 2): a dry build's failing units, with those files' real headers, go back for a revision; the best by fully loaded
  units is kept (`_echo.kept`). Saved `proposed`: nothing is built or served until the user accepts (Brain → Numbers:
  Accept and build; `echoctl brain numbers accept`). **Build** (`build.rs` + `load.rs`, Brain's `build_marts.py`
  ported: cells from A1 as openpyxl gives them — dates, time cells as seconds —, vintage order by the month in the file
  name, collisions merged by policy with `fact_merges`, `fact_versions` restated / conflict, every report in
  `fact_reports`, rollups `<rollup>`, derived `<derived>`, `build_audit.json` (ok / partial / zero / error per file and
  unit) and `coverage.json` (holes, expected roster)); facts in the store per client (`facts`, …). Rebuilt after a
  Brain sync when its workbooks, config or catalog changed (`numbers::refresh`, `inputs` hash). MCP `list_metrics`,
  `get_metric` (grain, entity / entity_like, month(s); `other_reported_values`, `restated` / `conflicting`,
  `merged_from`, `caveats` — notes written beside the table, Brain's `scan_notes`: a lone sentence ≥ 40 chars right of
  the header run past a blank column, in `fact_notes`), `get_metric_history` —
  accepted clients only. `entity_regex` (tolerant_long) cleans an entity's encoding by its first group. Measured on a sample set of reporting
  workbooks: with a Brain's own config Echo's build matched its facts value for value (the few left: files outside the
  folder); Claude's own proposal with repair rounds recovered about 90% of them in ~4 minutes. `echoctl brain numbers
  profile|propose|import|accept|build|show|list|get` (`--root` reads a folder instead of the client's threads').
- **History** (Brain's `supersede_before`, per thread: `Project.history_before`, Profile → History, `project set
  --history-before`): after each sync `store::supersede` marks the sections of documents, posts and mail dated before
  every one of their threads' dates `SUPERSEDED` (`valid_to` = the date; recorded in `date_superseded`, so only these
  marks are undone when a date moves or goes); calls aren't (a thread's Since decides its calls), undated documents
  never are. Search (`store::search_at`, `History`): latest (default — superseded hidden), all (MCP `latest_only:
  false`), or `as_of` a day (sections dated after it or superseded by then hidden). The Sources tab tags them "history".
- **Meeting transcripts saved as Word** (`office::teams_turns`): Teams' (Brain's `read_teams_docx` — one paragraph a
  turn: speaker run(s), an `M:SS` run, the words; "started / stopped transcription" dropped) and Otter's ("Name  00:03"
  on its own line, the words after) → packed turns like a .vtt (`text::pack`), the title / date / duration preface kept
  (it dates them); method `meeting-transcript`.
- **Strict routing** (`brain/access.rs`; as for a call's own jobs: never another profile): a call's text belongs to
  the profile pinned in its `meta.json` (`ai::call_profile_id`; unreadable → nowhere), mail, posts and folder documents
  to their threads' profiles (a thread's own, else the default). The MCP server takes its reader (`echoctl brain mcp
  --profile P [--client C | --thread T…] [--call call/<stamp>]`; none given: the default profile's) and every tool
  answers from that profile's sources only — searches filtered before the cut, documents / evidence / related of
  others "not found", scopes and numbers listed only where allowed. Echo-folder session: its profile; a call's
  session: the call's profile and its client, else its threads, else the call itself (`terminal::brain_mcp_config`,
  a config file per access). The `<echo_view>` hook drops the open call and the selection for a session of another
  call or another profile (`ECHOCTL_SESSION_PROFILE`, `view::restrict`). Background jobs run on the default profile
  and take only its sources: insights, taxonomy builds and tagging, the page reader (a folder whose threads run on
  other profiles is indexed from text, never read — and not left pending), the numbers proposal (refused unless all
  the client's threads run on it). A thread's sync makes evidence only of calls on the thread's profile.
- **Closed threads are the archive** (#55; never dropped silently): a closed thread's mail, posts and folder documents
  stay under it (`sources::with_sources`, `files::with_folders`: only open threads read on; a source no thread has any
  more is still removed, and a closed thread's read state is kept, so reopening reads on from there); its calls stay
  as they are. A section whose threads are all closed (`store::archived_of`, `projects::closed_threads` = `{id, name,
  stage, closed_at}`) is found by search with `closed` and weighs half (`ARCHIVE_WEIGHT`) — not within that thread's own
  scope; the doc panel says "closed thread · …". Brain People leaves someone seen only in closed threads off the map
  (`people::everyone` → `archived`) until the switch above it, Show people from closed threads (`S.br.pClosed`), puts
  them on it greyed, with their closed threads — or the scope is that thread; a term or topic mentioned
  only there is `archived` in the taxonomy view → the Terms tab's folded "Closed threads" group, and out of a new call's
  hint (`terms_for_threads`); the `<echo_brain>` advert lists open threads and one "Archive: N closed threads — …" line.
  Everything is computed from the stages, so reopening a thread makes it current again with nothing read again.
  A closed thread keeps what it had and takes nothing read after it closed (mail / posts: `had`; a folder shared with an
  open thread is read under the open threads only); a sender decision (`reread`) leaves an all-closed document alone; a
  client's numbers keep its closed threads' workbooks. Known gap: a `READER` bump drops every mail / channel document and
  read state, a closed thread's archive included (reopened, it reads its window again). Not done yet: Forget on close.
- **Off switch** (Settings → Brain → Use the Brain; `brain.json` `enabled`, default OFF — a fresh install and an update both start with it off until the user turns it on; `echoctl brain set --enabled
  false`; IPC `settings_set brain_enabled`): off, nothing is read, indexed, tagged or summarised (`brain::sync` refuses,
  the scheduler skips), the window shows no Brain (activity, ⌘ numbers and ⌘K, Also discussed, Insights, Mentioned also
  in, Topics & terms → the glossary, no channel / mail / folder sources to add; `/api/state` `brain`), Claude's sessions
  get no echo-brain server, transcription hints come from glossaries; the store stays on disk as it was.
- **Topics** is one tree: the practice's topics by size (a bar, the share), each with its subtopics and the clients'
  topics under it (a chip names the client), then "Only <client>" for a client's own; definitions and synonyms under the
  name; drift's catch-all flagged in place. Above it, **Topic builds**: one line per layer saying in plain words what is wrong and the one thing to do — Needs a rebuild (why; Rebuild · Accept when provisional · Not now), Waiting for your review (Accept), Rebuild put off (snoozed), Not built from calls yet, Nothing to do — with Tag again and Rebuild always there as quiet links, **Tag again** (IPC `brain_retag_topics` → `classify(layer, again)`, background) and
  **Rebuild** (`brain_rebuild_topics` → `induce`, provisional) — and the suggested **moves** (Move up / down · Keep it);
  **Sync now** reads new sources and tags their sections (a manual sync tags too, as the scheduler's); what runs is
  `/api/brain` `topic_jobs`, a notification says when it ends. (A sunburst was tried and dropped: too hard to read.)
  **Terms → the dictionary** (2026-11-02): a term is a named thing with a name, **synonyms** (`Term.synonyms`: other
  names, abbreviations, older names — search, mentions, Claude; renames and merges keep the old name here) and
  **spellings** (`variants`: how it is written or misheard — the transcription hint and mentions), a **group** from a
  fixed list (`GROUPS`: organisation, product, initiative, document, metric, method, concept, other; `Term.group`, else
  `group_of(kind)`; the kind stays its subtype; "learned" is a source, not a group) and a scope like topics' (the
  practice or a client; one name in several clients → a Move up, done as the practice's term with the others merged
  in). Old terms load split (`split_variants`, every load until saved: an abbreviation or a name under 0.75 alike →
  a synonym; a near spelling, or another script than the name's — "Эко" — → a spelling). Edit ops: `set` (synonyms,
  variants, group), `split` (a name becomes a term of its own), `rename`, `merge` (`into_layer: shared`), `scope`. The
  view gives each term its topics and each topic its terms (sections they share, no model). The Terms tab is the
  dictionary: search over every name, groups with counts, A–Z within; the panel defines, regroups, splits, merges,
  moves. Measured on a working store (a copy): free kinds folded into 8 groups, names shared by clients offered up;
  merging one (in 3 layers) dropped only its duplicate mentions.
**Graphs** (2026-11-02): the Brain's Graph and People are drawn by Sigma.js 4 (WebGL) over graphology, vendored in
`ui/vendor/` (`GraphView` in app.js): laid out once per data set — ForceAtlas2 run in the page (never its worker:
the CSP), then noverlap —, kept across renders (the camera too), labels where they fit (the biggest first), hover lights
a node's neighbours. Search: the Graph's box lights topics and sources by name as you type and, on Enter, asks the Brain
(`/api/brain/search` = `store::search_in`, words and meaning, the scope) and lights the sources of its hits; People's
box lights people by name, address, title, organisation, workstream and role. Matches and their neighbours stay lit,
the rest dims (plain hex colours: WebGL reads no CSS), the camera flies to them; Esc clears. The hovered node's label sits on sigma's backdrop card, white by default: given the theme's colours in the node reducer (dark `#26262e`, light white; a 1 px border, radius 6) and hidden on a dimmed node; a hovered neighbour's label is `--text-2`; a change of the system's appearance draws the graphs again (`GraphView.retheme`).
**The window**: the **Brain** activity (`#brain/<tab>`: Topics — each layer with its summary, provisional banner and
  Accept, the tree with counts and threads; Terms; Sources; Coverage; Graph — topics on a ring grouped and coloured by
  client, sources inside at the weighted middle of their topics, related sources dashed; a click selects a node — a
  topic opens its panel, a source the `brain-doc` panel (`/api/brain/doc`, `views::doc`: kind, threads, Open — the call,
  Teams / Outlook, the file —, topics and terms, first sections, related sources), its edges brought up, the rest dimmed; a scope
  picker: everything, a client, a thread), the topic or term in the right panel (`brain-node`: rename, merge, move,
  remove — IPC `brain_taxonomy` —, its latest sections, `/api/brain/node`); a call's **Also discussed** (right panel
  `also`, `/api/brain/also/<call>`: per moment, the nearest elsewhere — another client's only ≥ 0.75 —, topic chips); a
  thread's Profile **Topics & terms** (its topics and terms from the Brain; + Term pins one in its client's layer).
  A work item's details show **Mentioned also in** (`/api/brain/mentions`, `views::mentions`: for each of its quotes the
  section saying it and its nearest sections elsewhere in the thread, else a search of its title in the thread; the
  calls it cites left out). **Claude about this thread / client** (thread ⋯ menu, client header) opens the Echo-folder
  session; `<echo_view>` names the client or thread with its id and tells Claude to scope Brain searches by it.
- **Taxonomy** (`taxonomy.rs`; it supersedes thread glossaries and the learned vocabulary): one per user —
  topics (L1/L2) and terms (name, kind, variants, description, pinned threads, always) — versions in
  `<home>/brain/taxonomy/` (`vN.json`, `current.json`, `decisions.jsonl`). Built by `echoctl brain taxonomy build`: each
  call's minutes mapped (4 calls a request), names clustered (Brain's ratio ≥ 0.86), one judge request, saved
  provisional, then every section classified (40 a request, 0–3 labels, `__no_topic__`); requests are no-tool Claude
  runs on the default profile's minutes model (`livenotes::complete_text`, 5 min each), every answer checked in Rust.
  Term mentions are found without a model (whole words, name or variant) after each version and sync; the app tags new
  sections after a sync. **Learned vocabulary** lives here (`Term.learned`: subjects, people, calls — the old
  `config.learned` scope rules, `vocab::applies`): a minutes run's `vocab add` → `taxonomy::learn` (the call's
  client's layer, else shared; pinned to its threads; merged into a term of that name or variant, else kind
  `learned`; one version per call); the old list moved once (`vocab::migrate_learned`, at the scheduler's start and
  in `echoctl vocab`, never inside a run; a copy in `.echo/learned-before-taxonomy.json`), `vocab::learned_all`
  reading both meanwhile; the Vocabulary panel forgets one (`forget_learned`: a learned-only term goes, a known one
  loses its scope). Transcription's, so it works with the Brain switched off; thread glossaries no longer learn
  them. Measured: the learned terms spread over the clients' layers and the shared one, most merged into terms the
  taxonomy already knew.
  sections after a sync. The user's edits (`taxonomy edit`: accept, add, rename, merge, move, remove, set) make a new
  version; renames and merges keep tags through aliases, removals drop them. Transcription's hint reads the terms pinned
  to or mentioned in the call's threads (`projects::terms_for_meeting`). Measured: 11 calls → 19 topics, 102 terms, 285 of
  305 sections tagged (15 small talk, 5 nothing fits). Measured on this Mac (Ollama `qwen3-embedding`, 4096-d): 11 calls →
  305 sections, first embedding 4 min; English and Russian queries find their moments; Claude Code with only this
  server answered a question across two calls with times and said what wasn't modeled.

## Echo plugins (`plugins.rs`, `jshost.rs`, `claudejob.rs`; plan: `docs/plugins-design.md`, v4)
Not Claude Code plugins: packages that extend Echo itself **with their own code** (v4, the user's call 2026-11-03, after
dsh-desktop's cordis plugins): `echo-plugin.json` (id, name, version, `echo`, summary, `uses` — what it touches, for the
install sheet —, `main`, `client`, `settings` of type string | bool | folder | secret) + `main.js` (the **host half**) +
`client.js` (the **client half**) + skills, templates… Installing is trusting its code (`plugins::sheet` puts "Runs its
own code in Echo / the window, with your rights" first). Echo keeps only primitives; a plugin's features live in its
package. **Host half** (`jshost.rs`, `js` feature: QuickJS via `rquickjs`, C — off in the Windows cross-check): one host
thread, one context per enabled plugin, loaded at app start and after install / switch / remove (`jshost::sync`); its
ES module's `apply(ctx)` gets Echo's services over two Rust functions (`__host` at once, `__hostAsync` → a Promise settled
from a worker thread) wrapped by `PRELUDE`: `ctx.command(name, fn)` (async allowed; called by `jshost::call` — the window's
`GET /api/plugin/<id>/call/<cmd>?args=`, IPC `plugin_call` from Inbox buttons, `echoctl plugin call <id> <cmd> [json]`, which
never runs timers: `no_timers`), `ctx.every(sec, fn)` (ticks each second in the app), `ctx.on(event)`, `ctx.store` (its
folder `<home>/.echo/plugins/<id>/`, relative paths only), `ctx.package` (its installed files, read-only), `ctx.settings` (get / set; secrets 0600),
`ctx.http.fetch` (HTTPS to its manifest's hosts), `ctx.markdown` (Echo's safe writer), `ctx.notify`, `ctx.inbox.set/clear` (`inbox.json` → Needs you items whose buttons
call its commands), `ctx.profiles`, `ctx.deliver.status/send`, `ctx.claude.run({prompt, title, profile, model, tools,
dir})` → `{answer, model, usage}` (`claudejob.rs`: headless on one of the user's AI profiles, the Brain narrowed to it;
`tools` read = the guard's `RUN_MODE=routine` — the data folder, the run's own tool-results, read-only
`guard::ROUTINE_COMMANDS` — the CLI lets such a run read projects, clients, orgs and a ledger's tree, `routine_reads` —, the Brain —, folder = the user's settings with dontAsk, skip; Echo adds the time and how to
reach its data; the CLI is the plugin launcher with `ECHOCTL_BIN` = this Echo; `usage` = tokens by kind, cost,
duration, turns from the result event, `projectsync::finish_run_result`). 64 MB, 5 s of JavaScript per slice.
**Client half** (app.js `Plugins`, `loadPluginClients`): each enabled plugin's `client` imported as a module from
`/plugin/<id>/<file>` (`window::plugin_file`, its own files only) into the window's own page (trusted, as dsh-desktop);
`apply(ctx)`: `ctx.call`, `ctx.settings.section({id, label, icon, badge, render})` (its own Settings section under "From
plugins", hash `#settings/<plugin>:<id>`), `ctx.settings.add` (its card in Settings → Plugins), `ctx.panels.add` (+
`ctx.showPanel`), `ctx.slots.add(slot, {label, icon, when, run})` (`SLOTS`: `call.menu`, `calendar.event`,
`task.actions`), `ctx.palette.add`, `ctx.ui` (Echo's helpers), `ctx.where`, `ctx.go`, `ctx.render`; switched off or removed →
`unloadPlugin` takes all of it out. **Marketplaces**: git repositories with `echo-marketplace.json` (name, `featured`,
plugins: `{id, source: "./path" | {github, ref, path}, categories}`); ours, `onetest-ai/echo-plugins` (public;
`ECHOCTL_PLUGIN_MARKET` points it elsewhere), built in (hidden, never removed); the user adds any number. Fetching
(`refresh`, never the page; again when the Plugins page asks and the last read is 30 min old or by another Echo version — `Catalog.echo_version`): GitHub `owner/repo[@ref]` → the commit (`curl`, else `gh`) → its tarball unpacked by `tar`;
a git URL (`#ref`) → `git clone --depth 1`; a folder read where it is; every tree checked (`check_tree`: no links,
≤ 5000 files / 64 MB a marketplace, ≤ 400 / 20 MB a plugin). Files: `<config dir>/plugins.json`,
`<config dir>/plugins/markets/<key>/`, `<config dir>/plugins/installed/<id>/<version>/`; settings per data folder
`<home>/.echo/plugins/<id>/settings.json`, secrets `<config dir>/plugins/secrets/<id>.json` (0600). Install = the sheet's
lines agreed; an update asking for more waits for a new agreement. Settings → Plugins (designed on the Workbench canvas: Installed — a row each with Update / problem pills and its switch,
the chosen one on the right with what it can do, its settings, Open its settings, Remove; Discover — categories, Featured,
cards, the install sheet over the page saying who listed it; Marketplaces — a row each, Read again, ⋯ Hide / Remove, Add;
IPC `plugin_op`); `echoctl plugin list|install|update|remove|enable|disable|set|call|marketplace …`.
**Channels** (`ctx.sessions.channel({plugin, env})`, only the channel the manifest's `uses.channel` names;
`jshost::channels`): the Echo-folder session starts with `--channels plugin:<spec>` and the env given — no `CLAUDE*` /
`ANTHROPIC*` keys. `ctx.claude.hasPlugin` / `installPlugin` (`claude plugin list --json` / `install --scope user` through the
profile: on, at user scope or this data folder's project), `ctx.system.which`. **Telegram** (its package, with its bot): writes the
channel's `access.json` in its store — `allowlist`, only the paired chat id, `TELEGRAM_ACCESS_MODE=static` (no pairing
codes) — and registers the channel; its Settings section checks the bot, Bun and Claude Code's
`telegram@claude-plugins-official`. Measured 2026-11-03 (personal Max account): a message reaches the open session, the
bot reacts 👀 and Claude answers through the channel; it still works after `--resume`; messages sent while the session
was closed aren't read. A Claude Code started from inside another (`CLAUDE_CODE_CHILD_SESSION` inherited) keeps no
transcript, so it can't be resumed.
**What plugins may reach beyond their store** (`grants.rs`, granted by the user or kept by Echo, never written by the plugin):
**folders** the user picked in Echo's own picker (`ctx.system.pickFolder` → `<config dir>/plugins/grants/<id>.json`;
`ECHOCTL_PICK_FOLDER` answers it in tests) — `ctx.fs.walk / list / read` only inside them (plain relative paths, links
not followed, a link resolving out refused, the resolved path is what is opened, Windows device names refused;
`uses.folders`, read-only; `ECHOCTL_PICK_FOLDER` only in debug builds); **Echo's sessions** (`uses.sessions`, each part
asked for: `skills`, `read`, `hints`): skills as text (`ctx.sessions.skills([{name, description, body}])` — Echo writes
the Claude Code plugin itself in `<config dir>/plugins/session/<id>/`: `plugin.json` `echo-<id>` + `SKILL.md`s with only
name and description, `!`…`` refused, so no hooks / MCP / agents ride along) given to the Echo-folder session and to
`claudejob` runs with `--plugin-dir` — never the user's Claude Code — and granted folders they may read
(`ctx.sessions.read`: `--add-dir` in the folder session; a read-only routine's guard reads `RUN_READ` and loads only those
skills, by full name `echo-<id>:<skill>`, `RUN_SKILLS`);
**hints** (`ctx.view.hints`, `<config dir>/plugins/hints/<id>.json`): a line in `<echo_view>` when that client or thread
(or a thread of that client) is open — one plain line, no tags or breaks; `ctx.echo.read('clients' | 'threads')` (only
the kinds `uses.echo.read` names). All of it only for enabled plugins; a reload drops what the old version added; removing
a plugin deletes its grants, hints and session plugin. The client half: `ctx.listen` / `ctx.ui.style`, taken out when it
unloads (else an update doubles every click). `ctx.claude.run({tools: 'none'})` = no tools at all.
**Obsidian** (its package; plan `docs/obsidian-design.md`, screens on the Workbench canvas): the user's own vaults, never
an Echo-made one. + Vault → the picker → a survey (no model: folders, frontmatter keys, tags, links, `.obsidian` daily /
templates folders) → one Claude run with no tools → a description in parts (**When Claude looks here** — one line ≤ 250
—, what it is, where things are, how notes are written, what Claude can do, leave alone) + which clients / threads it is
for → the user corrects it and confirms (Inbox until then; one analysis per vault at a time). Then **one skill**, `obsidian-vaults` (its description names
the vaults), a catalog `vaults.md` and each description — rebuilt on every add, remove, re-read and edit; a re-read waits
beside the user's description (`proposed.md`); a vault whose folder is gone (a drive not mounted) is left out.
**The Obsidian activity** (phase B; `ctx.activities.add` — a plugin's activity in the bar under Echo's, its route
`#<plugin>:<id>/…` in `ctx.where().route`, `render()` + `after()` to draw; `ctx.ui.graph` = Echo's GraphView,
`ctx.ui.newGraph()`): a vault browsed as Obsidian shows it — Files | Tags tree (left-alone folders greyed), notes as
tabs with their properties (a property's `[[link]]` opens its note) and body through Echo's safe Markdown writer
(wikilinks become in-place links, a missing one dotted), Open in Obsidian, the local graph, backlinks with their line,
tags, In Echo (a client or thread of the same name); a Graph tab by Links | Tags | Both, coloured by folder or tag, orphans
hidden. The host half keeps an index per vault (frontmatter, tags, links and the line each sits on), re-read only for files whose
time or size changed, in batches through `ctx.fs.readMany` (async: off the plugin's 5 s slice), one build per vault at a
time. Wikilinks pass Echo's writer as markers of that call only and become links in text nodes (never code or attributes).
A vault is never indexed into the Brain: vault and Brain stay apart (the user, 2026-11-03). Measured on a fake vault: the analysis in ~12 s; Claude Code with
the skill and `--add-dir` answered from the vault and named the notes.
**Scheduler** (its package in `onetest-ai/echo-plugins/plugins/scheduler`): routines = the user's prompts on a schedule
(`routines/<id>.md` in its store, templates in its package), runs with tokens and cost, its own Settings section; the
Telegram and Obsidian plugins wait for `channel`, `brain_source`, `export`, `sessions`.
**Delivery** (`deliver.rs`; to the user — mail also to the addresses set on a routine, `mail_to`; never an address a
model picked): Echo's own ways are Teams = Graph `POST /chats/48:notes/messages` (the chat with yourself) and mail =
`POST /me/sendMail` to `/me`'s own address, not saved in Sent (`graph::post`, `graph::my_address`; mail to others is kept
in Sent and its foot says who sent it), from `minutes::markdown_html`. A mail is Echo's (`mail_html`, `MAIL_CSS`: the
minutes page — the octopus, an inline `cid:` attachment, "Echo" and the day above one plain card, Echo's type, violet only
for links, code and the AI mark; no grey page around the card and no decoration — a top bar, section markers — the user
called AI-slop; Outlook recolours after the CSS, so every text is Echo's ink, rank by size and weight, never a lighter
grey, and only saturated fills — tables are reports: a violet head row with white text, faint zebra rows marked in the
HTML (`zebra`), horizontal rules; checked in Outlook on the web, light and dark). Anything a model wrote carries
`ai_notice` in every channel. **Other ways are plugins'**: `ctx.deliver.provide(kind, {label, send})` in a host half
whose manifest names the kind (`uses.deliver_provider.kinds`), its readiness told with `ctx.deliver.ready`
(`jshost::providers`); `deliver::send` hands such a message to that plugin (`jshost::provider_send` → its `__deliver`
command). Telegram is one (its package: bot token as its secret setting, pairing, sending through `ctx.http.fetch` to
`api.telegram.org` only — `uses.network.hosts` —, Markdown → Telegram HTML with tables as lines). Settings → **Sends to you** (its own section, `#settings/deliver`, `/api/deliver`) lists Teams, Mail and the plugins' ways; `echoctl deliver status|test <kind>|send`.

## The mod: a status line (and automation)
`plugin/hooks/register.tsx` is a Claude Code mod (state contract `plugin/types/index.d.ts`, pure helpers
`hooks/view.ts`) and only a **status line** above the prompt (2026-11-03, the user's call: the live pane and its call
browser went — Echo's window shows calls, live transcripts, live notes and minutes). At session start it asks the CLI
for the data folder (`echoctl app` → `home`) and starts the app if it is set up but not running. While the app runs:
**Echo** (hotkey e: `echoctl app open [--call <the recording>]`) + "● Recording · title · N min" (· 🔇 muted), "◌ Armed",
or "Echo · next: 18:30 …". Every 1 s it tails `events.jsonl` (from the end, never
replays; no `mod.json` heartbeat any more — nothing read it): muted / unmuted for the line, toasts for a failed transcription, an error, an expired arm. It asks for no
minutes (`pending --for` / `notes --auto` stay for older mods, and refuse) and never calls a model. `echoctl live-replay
<wav> <dir>` (hidden) plays a WAV in as a growing recording (10×) and writes the transcript, as after a call — only into
a new or empty folder, or one it marked (`.replay`), and only once transcription can run (never a call's folder).
**Where the plugin goes**: `echoctl claude connect` / tray → Claude Code → Connect installs it in the **project scope of
the data folder** by default (where Echo's own Claude sessions run; `--scope user` for every session); a folder session on
a `claude` / `local` profile reads only its own settings, so Echo passes the plugin shipped with the app
(`--plugin-dir`, `notes::plugin_dir`). "Claude Code ✓" counts a user-scope install or one whose folder is the data folder
(after `home set` Connect is offered again).
**Is Claude Code really set up** (`claude_setup.rs`, 2026-11-04: found isn't working): `check()` = it starts (`claude
--version`, 15 s, its own folder first on the PATH — an npm install is a node script), a terminal finds it (the login
shell's `command -v claude` between markers; Windows: the PATH), and Claude Code lists Echo's plugin where Echo's
sessions run (its record in `CLAUDE_CONFIG_DIR` when set); `status()` keeps it ten minutes, checks again at once
after Check again / Connect / Add to PATH — once more after a check already running — and recounts in the background (`/api/settings` `claude_code`, Settings →
Health's Claude Code rows, the Settings badge, General's plugin row — no longer the bundled copy's presence).
`connect_claude` refuses a `claude` that doesn't start and, after Claude Code says installed, checks its own list and
the files (else an error naming the command to run). **Add to PATH** / `echoctl claude path`: one marked line, the folder single-quoted, in `$ZDOTDIR/.zshrc`, or bash's
first existing of `.bash_profile` / `.bash_login` / `.profile` (Windows: the user PATH, unexpanded `REG_EXPAND_SZ`, and
the change broadcast — `app::add_to_user_path`, `install_cli` too; the check reads the stored PATH, not Echo's own). `echoctl claude` prints the check. User guide: `docs/install.md`.
Mute sync: while recording, the watcher polls Teams' microphone button via Accessibility / UI
Automation (`ax::MuteWatch`, label "Unmute…" ⇒ muted) every 400 ms; muted ⇒ `audio::mic::MUTED` makes the mic
track record silence; events `mic_muted`/`mic_unmuted` drive the status line. `config.follow_app_mute`
(default on). `echoctl ax-mic` (hidden) shows what the detector sees.
A call can pin languages (`arm --languages uk`, tray Record ▸ Only …, the window's Record
button's language menu — a native `<select>` over its right part; default English, remembered
in the webview, "Detect" = the configured list; scheduled / joined meetings carry it as
`MeetingContext.languages`) → `meta.json.languages` overrides the configured list in live and
final transcription.

## AI profiles and live notes (`ai.rs`, `livenotes.rs`; design + measurements: `docs/live-notes-design.md`)
A profile = which model and account an AI job uses, per computer in `<config dir>/ai.json` (`default`,
`live_notes` — off until turned on, `profiles`; loading refuses invalid or duplicate ids, saving is atomic and 0600 from the start).
Kinds: `system` (built in, never stored: Claude Code as the user set it up — login shell, all settings,
`config.notes_model` / `live_model`), `local` (a Messages-API server at `base_url`: Ollama on this computer; nothing
checks it is local — the window names the host when it isn't), `claude` (a claude.ai account in its own Claude Code
folder `<config dir>/claude/<id>`, signed in once; the first account seen (email · organisation, not the plan) is
recorded, and another one later stops jobs until it is accepted: window Use X / tray → AI for calls → Use X /
`ai accept`).
**Signing in** (`signin.rs`, the app): window Sign in… / tray → AI for calls → Sign in…, or choosing a signed-out
Claude profile in one of the window's menus (Live header, Record, Write minutes: `choose` + `ensure_request` / `applied` — only an explicit
`loggedIn: false` opens the browser, and only if that choice is still the latest for its purpose when the check ends
and right before the browser opens; a call's choice counts from when it reaches the app, its check runs once it is
applied) → Echo runs `claude auth login --claudeai [--email] [--sso]` (`Profile.email`, else the recorded account's;
`Profile.sso`; `ai add-claude --email --sso`, `ai set <id>`) through `ai::claude_command`, no terminal: the browser
opens Claude's sign-in page, which calls Claude Code back on `http://localhost:<port>/callback`, and the child exits
after that (measured); then one `auth status` judged against the settings as they are now (`ai::settle`) → "Signed in
as …", and live notes waiting for it go on at once (`livenotes::signed_in`). 10 min at most; Cancel; quitting stops it.
The window's Claude sessions on the profile hold its lease while they run: a sign-in closes them first
(`terminal::close_profile`; they resume by id), and if the lease is still held after 10 s the error names a session that
didn't stop (`terminal::holds_profile`) apart from minutes or another sign-in (#16).
A reader/writer lock per profile, `<config dir>/claude/<id>.lock` (`app::FileLock` shared/exclusive): jobs and
`auth status` checks hold it shared (`ai::lease`; `for_call` → `Ready` keeps it for the job), a sign-in exclusively
(`ai::sign_in_lease`). Meanwhile `for_call` fails with `ai::SigningIn` (it never waits): live jobs retry — in the app
`livenotes::defer` holds them back until the sign-in ends, so they can't take the lock first — and a minutes run waits
(≤ 12 min). The lock's text is a token each sign-in rewrites; each process's readiness cache is keyed by it. A lock
released a moment ago can still look held while another thread starts a process (macOS): `lease` tries for 300 ms,
tests use `testenv::eventually`. Errors are typed (`ai::SignInNeeded`, through the cache too) →
`CallState.needs_sign_in` / notes-run `needs_login` with its `profile` → the window's Sign in… / Use X. The window's
and tray's state (`signin::SignInState`, in `ai::summary`) never runs a process; checks run in the background: app
start, window open, and when nothing current is known (none yet, a changed token, a model said signed out), at most
every 30 s; one that finds the profile usable again resumes what waited for it. `echoctl ai login <id>` signs in in
the terminal it was run from (foreground, inherited stdio — Claude Code's paste-code fallback works — Ctrl-C stops it),
under the same lock. Tests run a fake `claude` (`ai::fake`; a test never starts the real one).
No `ai.json` = only `system`: Claude Code launched as before, but
its minutes are gated like a local model's (below) and live notes are off. A call pins its profile in `meta.json`
`ai_profile` at recording start (explicit: `arm --ai`, the window's Record menu, a scheduled / joined meeting's
`MeetingContext.ai_profile`; else the default; `!unreadable` when `ai.json` can't be read; one rule, `ai::pin`, for
the watcher and `capture`). Every `ai.json` change goes through `Settings::update` (a lock + re-read, so one writer never
undoes another; a new default must name a profile). **Recording never depends
on any of this**: `arm` doesn't check the profile (only a typed `arm --ai` is checked by the CLI). Mid-call change →
`ai-profile.request` → the watcher (it owns `meta.json` until `finish`, which keeps a switch written straight to the
file before the heartbeat said the call records) → `livenotes::profile_changed`; afterwards `ai::set_call_profile`
writes it. Request files name the call; one retitled since is found by its stamp. Every `meta.json` change takes the call's `.meta.lock` (`calls::update_meta` /
`lock_meta`, the recording's writes too) and is atomic (`paths::write_atomic`: a temporary file of its own per write). **Routing is
strict** (`ai::for_call`): a missing profile, an unreadable `ai.json` or `meta.json`, a signed-out or other account →
the AI job fails with the reason; never another profile. `ai::claude_command` is how Echo runs Claude Code on call
text, checks a sign-in or signs in (`echoctl ai login` — `login_here` — and `claude connect` start it otherwise): `system` = the old
command; `local` / `claude` = after the login shell, every `ANTHROPIC_*`, `CLAUDE_CODE_USE_*`,
`CLAUDE_CODE_SKIP_*_AUTH`, `CLAUDE_CODE_OAUTH_*`, `CLAUDE_CODE_API_KEY_*`, `CLAUDE_CONFIG_DIR` unset (on Windows
whatever their case) (`ai::routing_var`, and the same families as a `sed` filter, `UNSET_ROUTING`, for the shell — change both), the
profile's values exported from `ECHOCTL_P_<name>` (never in the script's text), `NO_PROXY` for a loopback `local`,
`--setting-sources user` (not for `claude auth …`: it rejects the option) — measured: a project
`.claude/settings.json` `env` beats the process environment. Children get their own session (`ai::detach`:
`setsid`; a process group alone stops an interactive login shell started from a terminal on SIGTTOU) — not `ai login`'s
child: it stays in the terminal's foreground (it reads the paste code; Ctrl-C reaches it). Admin policy that
could route (managed settings files + `.d/`, macOS managed preferences, any Windows policy key, the profile folder's
cached `remote-settings.json`: a routing `env`, `apiKeyHelper`, `policyHelper(s)`, a source that can't be read) →
Claude Code runs for `local` / `claude` are refused (`ai::policy_conflict`); `local` live jobs (curl) don't start
Claude Code.
Every short model job (live notes, questions, the Brain's readers: `complete_claude_with`) runs with
`--settings <config dir>/lean-job-settings.json` (`claudeMdExcludes: ["**"]`) and `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`:
in the data folder (this repo's own) CLAUDE.md and memory made each 90-second update ~46K input tokens instead of
~2.5K (measured 2026-10-30: a 26-min call ≈ 16 updates). What a call's live jobs used is added up from Claude Code's
result envelope in `<call>/live-usage.json` (`livenotes::Usage`: jobs, input / cache read / cache write / output
tokens, cost) and shown in the Transcript tab's header. When the call is over its live notes are kept at the end of
`transcript.md` (`## Live notes`, marked a draft; `markdown_section` in `render_markdown`, and `refresh_transcript`
after a later update).
Live notes (`livenotes.rs`): one worker in the app; calls keyed by their folder stamp (`calls::stamp_of` / `by_stamp`:
retitling keeps it; two calls in the same second share one, and `by_stamp` then refuses to guess — the scheduler keeps
the folder it last saw). A summary ≤ every 90 s with ≥ 400 new characters (not bytes), from a cursor of complete
`live.jsonl` lines (advanced only when saved; backlog > 12K chars → newest part + a recorded gap), prompt = previous
notes + new lines, always English, JSON `{now, decisions, actions, questions}` validated (none of the four fields →
refused: it would wipe the draft) → `live-notes.json`. Asks (window, `echoctl live ask`, the pane): missed = last 5
min, question = last 10, ask = the call (24K) + notes → `live-asks.jsonl`; asks go before summaries; every question
ends as an answer or an error line (a refused one too), unless Echo quits first (questions wait in memory); one
whose call folder can't be found says so in the call's state. `local` jobs: a direct `POST /v1/messages` with `thinking:
disabled` through curl (`-q` first, no `-L`, `noproxy` for loopback; config on stdin) — Claude Code can't turn Qwen's
thinking off (~50 s vs ~7 s an update); other kinds: lean `claude -p --tools "" --system-prompt …
--strict-mcp-config --no-session-persistence`, text on stdin from a thread of its own (so the deadline and stopping cover it) (`system` from `notes_in`, else the data folder, so its `.claude/settings.json` applies — strictly, as its minutes:
an unreadable `config.json` or a `notes_in` that isn't a folder fails the job, `ai::system_folder`). Stopping: pause stops summaries (a question goes on), a profile change stops either
(a summary is dropped, a question asked again with the new profile), quit stops all — the worker sees the request
before and while a child runs and stops it (`ai::stop_child` / `stop_group`: TERM to its group, then KILL to whatever
of the group is left after 3 s — the leader exiting first doesn't end the watch; output a leftover process still
holds after the leader exited gets 2 s, then that group is stopped too). Turning live notes off stops the running job
and drops queued questions with that reason. `local` and `system` jobs share one
lock, `<config dir>/local-model.lock` (`app::FileLock`: busy vs a real I/O error), across live jobs, the app's minutes
and `notes --wait`; minutes on them wait while any call records or is transcribed — the run's message names it ("waits for the recording of “X” (started 10:01) to end, about 11:00" from its invite's end, "…for the transcript of “X”…", "…for the model to finish another job"; `notes-run.json` `wait: own | call | model`, its own call checked first and never skipped); on Claude Code as configured — not routed by its folder's settings to a model on this computer — the window offers **Write now** while it waits for another call (`may_start`) (IPC `notes_now` → `<call>/notes-now.request`, read by the waiting run, used up when it starts; never a local model, never its own call); the Inbox lists each run waiting for something it names (`minutes-wait:<stamp>`; not the moment every run spends starting; once it starts it leaves quietly, not Done) — "Minutes not written yet: X", what it waits for, Write now where it applies; a queued run whose owner died shows as failed. `live-state.json` mirrors the state
for the pane only (rewritten `ended` at app start). `echoctl live test <call> [--ask kind text]` (hidden) runs one job
right there. Measured on a Mac with Qwen 3.6 35B-A3B (Q4_K_M) through Ollama: summary 5–8 s (18 s cold), a question ~4 s;
Claude Code + haiku ~9 s. Unverified: jobs on two signed-in Claude folders side by side (the sign-ins themselves keep
separate Keychain items: checked), live jobs during real transcription load, Windows.

## Minutes from the app (`notes.rs`)
**Automatic** (`config.auto_minutes`, default on; tray → Claude Code → Write minutes automatically; `config
--auto-minutes`): the app's scheduler (`notes::start_scheduler`, `auto_tick` every 15 s) starts a call's minutes when
`notes::auto_due` says so — switch on and the call ended after `auto_minutes_since` (saved at the first start with the
switch on and at each off → on, so no backlog is sent); closed, transcribed, not being transcribed; never tried (no
`notes.md`, no `notes-run.json` but an automatic run whose owner died before it wrote anything — still `queued`, or
`running` with its Claude Code gone too —, no session's fresh claim); past the gate
(`calls::minutes_gate`, shared with `context`); and now ≥ max(end, `my-notes.md`'s mtime) + 15 min. Admission is
synchronous (`admit(dir, Intent::Auto)`: under the place and `.meta.lock`, `auto_due` again), and right before Claude
Code starts an automatic run checks `auto_due` again (its own record exempt); not due any more, or the app quitting
while it waits, or Claude Code stopped by the app quitting before it wrote the minutes → **withdrawn** (record and claim
removed: due again later). An admission that fails is logged (once per reason) and recorded as the call's failure after
4 ticks in a row, with a notification (missing Claude Code / plugin: at once); one that can't even be recorded is left
alone until Echo starts again, `auto` saying why (`notes::GAVE_UP`), as for a boundary that can't be saved. Automatic
minutes also wait while `my-notes.md` can't be read. An abandoned automatic run frees Echo's claim. The switch governs every automatic path (the mod's included).
**The countdown** (#42): the Minutes tab shows a ring with the minutes left, the time, what moved it (`auto.moved`: your notes'
edit, or Not yet), Write now and **Not yet ▾** — 15 minutes more or until I say (IPC `minutes_later {minutes: 15 | null | 0}` →
`<call>/minutes-later.json` `{until}` / `{manual: true}`, `notes::set_later`; `held` = until the user writes them); the calls
list shows ⏱ N min on a call ended in the last 2 hours (`minutes_at` in `/api/calls`), the status bar the next one.
The window: `auto` = `{due: now|at|off|before|not_ready|tried|held|gate|missing|error, at, moved, reason}` (`missing`: due, but
no Claude Code or plugin here, `notes::auto_missing` — no "minutes at" is promised then; `error`: no settings).
**Write minutes** (window Minutes / Actions tabs, the pane's buttons, `echoctl notes <call>`) is explicit
(`Intent::Explicit`: its prompt overrides the skill's short-call gate) and runs the same
`/echo:process-call` skill headless: `claude -p … --output-format stream-json`, through the
user's login shell (`$SHELL -l -i -c`, their env: gateways, `ANTHROPIC_*`, Bedrock/Vertex), cwd
= `notes_in` or the data folder (for `system` strictly: `ai::system_folder`), `--plugin-dir` = the plugin shipped in the app
(`Echo.app/Contents/Resources/plugin`; dev: repo `plugin/`; else the installed one), model =
the call's profile's minutes model (`ai::Profile::notes_model`, one answer for the run, the menu, the window and the
minutes record: `system` = `config.notes_model`, default sonnet, `default` = Claude Code's own; `claude` its own, default
sonnet; `local` its own, else its live model; menu → AI for calls → Minutes model for <the default profile>, `echoctl ai
set <id> --notes-model`). **The guard** (`guard.rs`, `echoctl run-guard`, hidden): a `PreToolUse` command hook,
`onFailure: block`, in the run's own settings file (`--settings <config dir>/minutes-run/<run>.json`, removed after the
run; with `disableAllHooks: false` and `claudeMdExcludes` for `calls/**`). It allows only `Bash` = `<cli> <one of
notes::RUN_COMMANDS> …` as plain words (no `$`, backtick or newline; none of `;&|<>(){}*?[]~#!` outside quotes),
Read inside the run's call folder (canonical: links and `..` followed) and of the previous minutes of the same meeting,
Glob/Grep with a path in the call folder (patterns: not absolute, no `~`, `..` or braces), Edit/Write of only
`notes.md` and `actions.new*.json` at the top of the call folder, never through a link (not Echo's own files —
`meta.json` would pin the call to another profile for later runs —, the user's notes, or an instruction file whose
`@imports` would load other files with no tool call), and Skill `echo:process-call`. Anything else, or anything it can't
judge (a panic too), exits 2. Its scope comes only from the run's environment (`ECHOCTL_RUN_CALL`, a folder with
`meta.json`; `ECHOCTL_RUN_PREVIOUS`; `ECHOCTL_RUN_CLIS`, required) — a settings `env` could set them too, but only the
user's own. Claude Code runs it before every tool call, also one that needs no permission, and a refusal beats any
allow — measured with 2.1.310 on fake data: a `cat` of another call with `Bash(cat:*)` allowed and a Read inside cwd
were refused, and the guard still ran under a project `disableAllHooks: true`; without `onFailure` a guard that can't
start lets the call through. With the built-in profile Claude Code's own setup still applies around it: the user's hooks
(settings, plugins) run beside the guard and see each step — `disableAllHooks: false` turns them on for the run, also
when the user had switched them off — and CLAUDE.md files load as in a session. A run whose `init` event lacks `claude_code_version` ≥ 2.1.295 is
stopped before its model's first turn. Admin policy that switches settings hooks off (`disableAllHooks`,
`allowManagedHooksOnly`, or a `policyHelper(s)` that may set them, in the managed settings files, macOS managed preferences, the Windows policy keys' `Settings`
JSON read through the registry API — one that can't be read refuses —, or the profile's cached `remote-settings.json`;
`system`: in every folder its Claude Code may use — the `CLAUDE_CONFIG_DIR` its login shell sets, asked in the run's
folder with a 15 s limit and read between two markers with the shell's own folder after its startup files (no bounded
answer refuses; a relative one is taken from that folder, where Claude Code starts), Echo's own, `~/.claude`) refuses the run (`ai::hooks_conflict`, which returns the folders it checked: the run's
log names them); one
Claude Code fetches fresh at startup is seen only from the next run (known residual). Second layer: `--tools
Bash,Read,Glob,Grep,Edit,Write,Skill`, `--strict-mcp-config`, `--permission-mode dontAsk`, allow rules for the call
folder only (Claude Code decides Write by the `Edit` rule — a path-scoped `Write(…)` rule never matched, measured), the
previous minutes and `Bash(<cli> <cmd>:*)`, no `--add-dir` (the rules reach outside the working folder), and deny rules
(`Edit` of any call's `my-notes.md` or `actions.json`). Inside a run (`ECHOCTL_HEADLESS`) the CLI works only on the run's
call, resolved (`calls::resolve_call` → `guard::own_call`, which returns the canonical path: `other/../own` would put the
frames' trash inside another call's folder; `vocab add` without `--call` = that call), `title` sets the title but leaves the
folder's name to the run's end (`rename_to_title` then, on every platform; a terminal's `echoctl title` while a run is on
the call waits for it too, `notes::run_active`, as the guard works in that path) and marks it `title_from_run` — previous
minutes are matched by `calls::previous_subject`, which ignores such a title (a run could otherwise pick which other
call's minutes a later run may read), `frames --keep` doesn't purge the trash (the
app does when the run ends, `calls::purge_trash`), and `vocab add --always`, `transcribe --keep-audio` / `--model` refuse (`notes::refused_in_run`). By design a run
still changes two things outside its call: vocabulary learned for this meeting (`config.learned`, scoped to its subject
and people) and pruned frames moved to the trash. Transcripts are others' words and the user's own default mode (auto /
bypass) and permissions must not apply. Action
items carry stable ids (`calls::actions` gives ids once to an older list, before any reader or run sees it); the skill
writes `actions.new.json` and runs `echoctl actions <call> --apply <file>` (`calls::apply_actions`: only a new list in the call's own folder, `actions.new*.json`; keeps ids and each known
item's current status — ticks made meanwhile survive —, records the writer in `.actions-generation` from
`ECHOCTL_RUN_ID`; an app run's prompt names its own file, `actions.new-<run>.json`: that list, left unapplied — a
weaker model stops one step short — the app applies as that run's, `notes::finish_actions`; never the generic
`actions.new.json`, which could be a session's; why it can't be applied goes into the run's message); a current list
that isn't a JSON list is set aside as `actions.unreadable-<HHMMSS>.json` when a run is admitted or a list applied
(`context` and the window say why meanwhile); ticks are `calls::set_action_status(id, status)`. A run is `done` only with `notes.md` written during
it and this run's `.actions-generation`; then `notes-run.json` `minutes: {model, at, sha256}` (carried over by later
runs) names the model on the page while `notes.md` still has that hash. `ECHOCTL_HEADLESS=1` inside: the mod
does nothing, `pending`/`claim` refuse. A session's claim is **live** (#43): `echoctl claim <call> --session <id>` records the session id and the Claude Code
process it runs under; it holds a call only while that process lives and works on it (the claim under 15 min old, or the
minutes, actions or log changed since) — a bare-time claim of an older plugin, or one whose session went away, holds
nothing, and `claim` without `--session` is refused ("too old: update and restart Claude Code"); the Minutes tab says
who holds it (`auto.held_by`) with Write minutes. A session's `claim` (by hand: `/echo:process-call`) fails only while Echo's run
of the call is on or another session's fresh claim holds a call without minutes; a claim left by a finished run (Echo's
keeps it once `done`) is taken over. The CLI hands the run to the running app
(`notes-<call>.request`) unless `--wait` (Ctrl-C there stops its Claude Code too: `notes::shutdown`). The run uses the call's AI profile (launcher, model, strictness: above).
The user's Write minutes takes over a session's fresh claim (no minutes yet); automatic runs still yield to it — an
older plugin's pane claimed every transcribed call itself and Write minutes was refused for 30 min with nobody writing
(2026-10-30). Older Echo plugin installs in Claude Code (`app::stale_plugins`, from `installed_plugins.json`) show in
Settings → Health and Needs you with Update it (IPC `update_plugins` → `app::update_plugins`: the marketplace, then
each install in its scope, a project's from its folder).
One run per call (`notes::admit`): the call's place first, an OS lock on `notes-run.lock` held by the owner for the
whole run (released by the OS if it dies; it moves with a renamed folder; dropped before the final rename for
Windows), then under it the claim (a session's fresh claim on a call without notes wins), then `queued`; a refusal
changes nothing. State in
`<call>/notes-run.json` (queued — waiting for the call to end / the local model —, running, done, skipped, failed,
needs_login — "Not logged in · Please run /login" etc., from a failed run only; a `local` profile's refused token is
`failed`, saying how to set it — `notes::conclude`); `pid` = the owner (the app or a `notes --wait`) for the whole
run, `child_pid` = Claude Code; a dead owner = interrupted, unless its Claude Code still runs (then it stays the only
writer); the claim is freed once nobody holds the place; every failure from `queued` on is recorded; a run refused
before it starts says why there too; the run finishes in the folder as named after Claude Code (a title the run set
is applied to the folder only then; a rename made outside the run meanwhile is followed by stamp); quitting the app stops minutes runs' Claude Code and starts no more (`notes::shutdown`: a child is started and
registered under the same lock). Readable log in
`<call>/claude.log` (window → Show log). Failure releases `.claimed`. After transcription the app notifies
"Transcript ready — minutes at HH:MM. Add your notes in Echo before then." (switch off: "click Write minutes"); a run
notifies its outcome once ("Minutes ready"); the watcher's own check only covers minutes a session wrote. `config.live_model` = the `system` profile's live model (default haiku).
A local notes model (`system` profile; a `local` profile does this itself): the run's cwd (`notes_in`, else the data folder) gets a `.claude/settings.json`
whose `env` points Claude Code at Ollama's Anthropic API (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`,
the default-model vars) and `notes_model` names the Ollama model (one with `num_ctx` ≥ 64K: Claude
Code's prompt + an hour's transcript). Claude Code warns `unrecognized_model` for any model it
doesn't know, so that counts as a failure only when the run failed (`model_rejected`).
Minutes in other shapes (a user template with tables or YAML frontmatter): `calls::recap_from_notes`
skips frontmatter, reads a Decision table's cells and drops placeholders (`TBD`, `_None recorded._`).

## Minutes page (`minutes.rs`)
The window's Minutes tab shows it rendered fresh (above), whatever this setting says; Open in browser / Show in
Finder write the file first. The file is opt-in (`config.minutes_page`; menu → Claude Code → Write a minutes page (HTML
file) too; `echoctl config --minutes-page true`): when minutes are written, `notes.md` → `<call>/<title>-minutes.html`, one
self-contained file — header block from the `> **Label:** value` lines under the title + a
frontmatter's type / facilitator, tables, `[mm:ss]` as timestamps, `![…](frames/x.jpg)` embedded
(base64) only where the minutes cite them (no section of uncited frames) — and each "On screen" line
"- [mm:ss] …" gets the kept frame of that moment under it (`attach_screens`: nearest within 30 s, each once, no caption), in Echo's design tokens and type
(`minutes::ECHO_CSS`, light and dark; the window's frame adds `EMBED_CSS`: no card, no title of its own), mermaid inlined only when a diagram is drawn
(`securityLevel: 'strict'`), click to enlarge, light/dark, print. A run from the app writes it
after `done` (under the final title); a session's skill runs `echoctl html <call> --auto` (writes
only when on); `echoctl html <call> [--open]` writes it any time. pulldown-cmark parses; our own
writer escapes everything (raw HTML shown as text, https links only, images only this call's
frames). A renamed call's old page (generator meta) is replaced, never another file.

## Robustness, per-call extras, updates
- Recovery at app start (`watch::recover`): recordings never closed get their WAV headers fixed
  (`audio::repair_wav`) and are transcribed; `.transcribing` holds the pid, stale markers are
  removed; failures leave `transcribe.error` (not retried). One full pass at a time. A marker
  whose process is still alive (a transcription child of the app that ran before) is watched
  until it exits, and the call is transcribed then if it left no transcript.
- Sleep = wall clock outrunning the monotonic clock (> 30 s) → the recording is closed.
- Track restarts back off 1 s → 30 s (slow after 20, reset after 5 min stable); the remote
  track reopens after 5 min of exact digital zeros (≤ 10×; also what revoked TCC looks like).
- A track whose writer fails (a full disk, a volume gone) says so at once: a `write_failed` track event, a
  `track_failed` event and one notification. `Recording::finish` closes both tracks whatever fails, tells the live
  worker to stop (`Live`'s `Drop` does too), dates the end, then returns the error; what was saved is transcribed.
- `<call>/ui.jsonl` (`ax::UiRecorder`, `config.capture_ui`, default on): Teams' call window
  (`ax::CallWindow`, both platforms: the window holding `microphone-button` — AXDOMIdentifier on
  macOS, Teams' web UI is the same in WebView2; never the main window with chats) text diffs
  every 1 s on the audio clock + `ui-tree-{10,120,600}s.json` — raw material for speaker names
  (participant tiles read via AX; no screenshots).
- `<call>/frames/*.jpg` + `frames.jsonl` (`frames.rs`, `config.capture_frames`, default off,
  macOS needs full Screen Recording + Accessibility): the call window every 2 s (macOS
  `screencapture -l` on its CGWindowID from `_AXUIElementGetWindow`; Windows `PrintWindow`),
  **only while someone else shares** (`ax::Sharing`, shared logic in `ax/mod.rs`): the stage's
  `popout-content-button` (any language; absent while you present) → frames, the English
  "Content shared by <name>" → `shared_by`; nothing while you present — Teams doesn't say which
  window. Kept when settled and new vs **every** kept frame: 128×72 thumbnail, ≥ 4 % new (a
  slide is ~30 % of the window; a repeated title slide ~1–2 %), ≥ 4 s apart (measured on
  Windows; same on macOS). ≤ 150 per call. A call with frames always gets notes (`context`'s gate).
  `context.frames` → the notes' "On screen" section; process-call then prunes to the cited
  frames (`echoctl frames <call> --keep …|none`), which **moves** the rest, with their index
  lines, to `<home>/.echo/trash/<call>.frames/` (purged after 30 days, also on each prune) —
  never deleted outright. Restoring them is manual: move the files back into `<call>/frames/`
  and append the lines to `frames.jsonl` (`echoctl restore` is for whole calls).
- Ad-hoc calls (no invite, no agenda match) → `watch::find_call_people`: the Teams call
  window's title (`ax::call_title`, retried ~1 min) → `ax::call_name` (first " | " part: the chat
  the call was started from) → `graph::chat_named` (a recent group chat with that topic, or a
  1:1 chat with that person) → `<call>/meeting.json` (`source: teams-call`: members with emails,
  topic as subject) + `chat.id` for chat capture (a call from a chat posts no "call started"
  event). No sign-in: the title alone, if it looks like a person's name. Only when no
  `meeting.json` exists. Dev: `echoctl call-people [--name …]`. Names found this way miss the
  in-call vocabulary hint (built at start) but name the remote side and fill the notes.
- `<call>/chat.json` (`watch::start_chat_capture`, `config.capture_chat`, default on with a
  Microsoft 365 sign-in): the meeting's Teams chat via Graph `Chat.Read` — chat id parsed from
  the join link (`19:meeting_…@thread.v2`); ad-hoc calls (no link) → `graph::find_call_chat`:
  the recently active chat whose "call started" system event precedes the recording with no
  "call ended" in between (retried 5 min), and within 3 min of when Teams' call timer says the
  call began (`ax::call_elapsed`: the call window's "36:28" text, counted from the call's start,
  not your join; any language) — a big meeting left hours ago never posted "call ended", and its
  chat was taken for a 1:1 call that posts no events (2026-10-15). Timer unreadable ⇒ the call must have
  started after you last left one (`call_ended` in `events.jsonl` followed by ≥ 60 s out of calls):
  no chat beats another meeting's. The title's view name comes
  off first ("Meeting compact view | Jane Doe | …" → Jane Doe). `calendar chat` backfills the
  same way (title in `ui-tree-10s.json`, timer in `ui.jsonl`). Kept: messages from 2 min before the call started to
  2 min after it ended (recurring meetings share one chat) — polled every 30 s, final read 60 s
  after the end. `[{t, from, text, files}]`, `t` on the audio clock; `<call>/chat.id`.
  `context.chat` → the notes' "In the chat" section; pane: Chat section (live) + Chat tab (`h`).
  `echoctl calendar chat <call>` backfills; hidden `calendar call-events` lists call events.
- Invariants from the code review: one app per user (`flock`/share-mode lock on
  `<config dir>/app.lock`, not just the heartbeat); `Config::save` is atomic and nothing saves
  after a failed load; `transcribe` refuses a call still recording and takes `.transcribing`
  exclusively; call folders are never reused (`-2` suffix); release tags must be plain semver
  and reach the swap script only as `"$1"…` arguments; Windows updates (unsigned) install only an MSI
  matching the release's SHA-256 digest; the transcription worker is never joined on the watcher's thread.
- Updates (`update.rs`): GitHub release check at start + every 6 h (public API, else `gh`);
  `prepare` downloads, verifies and stages (mounts the dmg, requires our team id signature), `commit` schedules the
  swap after exit — only once the window's notes are saved and Echo is stopping (`app update` too: after `app quit`,
  which saves a recording), and from the tray not while a call is recorded (checked again in `commit`: one may have
  started during the save or a question). Windows:
  the release's MSI (offered only to Echo in `%LOCALAPPDATA%\Programs\echo`), checked against
  the asset's `digest` from the API (required: the builds are unsigned), run by `msiexec /qn`
  (per-user, no elevation; the MSI quits Echo, replaces the files, starts it). Every platform
  checks the digest when the API gives one. Code signing would add a publisher check on top.
- `me.md` profile (About me, `calls::me_profile`) → `context.me_profile`; read in parts by kind (`calls::me_parts`:
  blocks by heading or "Label:" line — identity, people, terms, patterns ("How I work"), style): the threads' sync
  gets all of it (`project ledger <id> context` and `people-context` `me_profile`, `verify-prep`'s answer
  `me_patterns`; EXTRACT / VERIFY / PEOPLE apply the patterns as defaults, the words win), Claude in the window too (a
  `SessionStart` hook — startup, compact, clear; a resume only when it changed — adds `<about_me>`), live notes and
  questions who I am / people / terms (`livenotes::call_header`, ≤ 1500 chars), the Brain's quarantine reader people and
  terms only, for spelling (its cache key carries their hash: an edit reads mail and posts again); Settings → About me
  says so; `echoctl compose` opens a mailto draft (never sends);
  `calls::recap` (deterministic "last time" bullets) in the Record dialog and Upcoming menu;
  tray "Add a note…" while recording; "Minutes ready" notification.
- Minutes bullets carry `[mm:ss]`; the window's page and the pane turn them into jumps; `actions.json` items have
  `id` + `t` + `status` (todo/done, ticked in the window and the pane by id). Live quick actions: What did I miss / Suggest
  a question (the call's AI profile, its live model, over the last 5/10 min; `livenotes.rs`).

## Transcription
**One pass, during the call** (`live.rs`): a worker reads `mic.wav` / `remote.wav` as the track
writers grow them (the WAVs on disk are the queue — nothing dropped when decoding falls behind;
≤ 60 s buffered per track; 4 threads) and decodes each finished utterance with the final
transcriber (`transcribe::final_transcriber`) → `live.jsonl`. When the recording stops the
worker decodes the rest; `watch::finish_transcript` (holding `.transcribing`) writes the
transcript (`transcribe::write_transcript`: order, echo filter, names, audio deleted) — seconds
after the call. Whole-call step: echo cancellation — on speakers the mic is cancelled and
decoded again at the end; on headphones nothing is redone. Any failure, no model when the
recording started, recovery after a crash, and `echoctl transcribe` run the full pass over the
WAVs (same steps). Release build on this Mac: 115 s of speech in ~21 s per track (model load
included).
Transcription server (`remote.rs`, for laptops without a usable GPU): `transcribe::Engine` is
whisper.cpp with the models loaded (`utterance`: language ID + decode + guards); `Transcriber`
decodes on a local `Engine` or sends each utterance to a server (`remote::Client`: a URL,
`http://` or `https://` (ureq + rustls, the OS's certificate store), plus an API key in
`<config dir>/transcription-server.json`, 0600). Voice detection, the filters, echo
cancellation and the transcript stay on the client. Server down ⇒ the local model for 60 s,
then the server is tried again. No local model ⇒ wait for the server (≤ 15 min; the WAVs hold
the audio). A 401 or 400 is final. Server = any Echo with Settings → Transcribe for other
computers (`echoctl server share on` → `<config dir>/transcription-serve.json` {on, port 47800,
key}; the app's `remote::run_in_app` thread follows that file every 2 s). It is a plain-HTTP
std listener: ≤ 8 connections, one utterance at a time, its own active model loaded at the
first request and dropped after 10 min idle, a wrong key answered after 1 s. Protocol:
`GET /v1/hello`, `POST /v1/utterance` (u32 LE JSON length + `{languages, prev, prompt}` + PCM
s16le) → `{lang_id, lang, segs}`. The key is 16 characters without look-alike characters,
compared without case or dashes. Plain HTTP was the user's choice for the local network; https
works through a proxy. `echoctl server [status|use <url> --key|off|share on|off [--new-key]|run]`.
Tested on this Mac: a client with no model got the transcript 3 s after the end, using almost
no CPU of its own; with the server down, it decoded locally.
**Or an OpenAI-compatible server** (`ClientSettings.api: openai` + `model`; Settings → Transcription → Use a server →
OpenAI-compatible…, IPC `server_openai`; `echoctl server use <url> --openai [--model m] [--key k]`): Ollama with an
audio model (0.40.12+: `lfm3-audio`), Speaches, vLLM, LocalAI, parakeet-api. Connecting lists `/v1/models` (no model
named: the first that looks like speech-to-text) and sends a test tone; `words` records whether word timestamps came
back. Each utterance a WAV to `/v1/audio/transcriptions` (`verbose_json`, word + segment granularities); the language
pinned when the call has one, else the server's detection checked against the call's list (outside it → asked again
pinned to the track's last language, else the first); the hint as `prompt`, then a plain decode and
`transcribe::choose_hinted` as locally; Whisper's no-speech rule where segments carry it; same fallback to this
computer's model while it's down. Measured with Ollama 0.40.13 + lfm3-audio on a three-language test call: 0.23 s for a
6 s utterance, but no `language` in its answer (the `language` parameter is ignored: uk/ru can't be restricted), one
segment per request with no word times, and clearly weaker than Whisper turbo on Ukrainian (Russian letters in
Ukrainian words) and on terms ("service now", "RACI"); Whisper turbo got all but one product name and one Ukrainian word.
Speech engines: choosing a model chooses its engine (`models::Engine` on each catalog entry;
`transcribe::Engine` is an enum with the same `utterance` contract — another engine is one more
variant + catalog entries). Whisper (whisper.cpp, linked in) is described below. **Nemotron**
(`nemo.rs`, model `nemotron-3.5-0.6b`, NVIDIA's 0.6B RNN-T, 742 MB GGUF, OpenMDW license):
NeMo-Speech.cpp's C ABI (`nemo_speech/asr.h`) loaded at runtime with libloading — no link-time
dependency, and its ggml (shared libraries) stays apart from whisper.cpp's (static). Runtime:
`ECHOCTL_NEMO_DIR` → `Echo.app/Contents/Frameworks/nemo-speech` (macOS, Metal; signed by
package-macos.sh — the hardened runtime loads only our team's libraries) or `<app>\nemo-speech`
(Windows, the CPU build) → `<config dir>/nemo-speech`; `scripts/fetch-nemo-speech.sh` pins
v0.2.0 by SHA-256. A build without it hides the model. One offline `recognize` per utterance
(word times in ms → sentences). Language: one allowed language ⇒ pinned; else an `auto` decode
of the first 4 s, restricted + sticky as with Whisper, uk vs ru settled by the letters it wrote
(і ї є ґ / ы э ё ъ — its own tags say `ru-RU, uk-UA` for Ukrainian and it writes the first
words in Russian), then the utterance is decoded with that locale pinned. Vocabulary = boosted
phrases (boost 0.5: 0.5 fixes said terms and adds none, 1 garbles, 2–3 insert terms into unrelated speech — measured with the 24-term vocabulary), decoded plain too and kept only when every vocabulary term the boost adds sounds like what the plain decode heard there (`boost_is_heard`: Latin-letter edit distance ≤ half the term; Cyrillic transliterated) — unguarded, the boost wrote "Flow Studio" / "ServiceNow" dozens of times into a Russian call that never mentioned them (2026-10-13). Whisper's hint gets the same check on top of its own. It never translates and doesn't invent text in
silence like Whisper. This Mac: 115 s of speech in ~11 s on the CPU alone, ~3 s on Metal.
**Speaker detection** (`speakers.rs`, model `speakers-nemotron-3`: NVIDIA's Nemotron 3
Diarization, Sortformer, ≤ 8 speakers, 107 MB, through the same NeMo-Speech runtime; on
whenever it is downloaded, with either engine): in `write_transcript`, unless a 1:1 invite
names the remote side, the remote track is diarized once (preset `v3-offline` on the streaming
state machine, the WAV fed in 10 s pieces; a 30 s two-speaker test: 0.1 s) and each remote
segment gets the speaker overlapping it most, numbered by first appearance → `speaker_id` +
name "Speaker N". A sentence two people share is split between its words (`Segment.words`,
word times kept in memory only: Whisper tokens / Nemotron words; one-word runs the neighbour's
turn also covers join it; on overlapping speech the one who just started wins). Word times
are corrected per engine first (Whisper +0.3 s, Nemotron −0.4 s, measured against reference
turns) — without it the first words of each turn went to the previous speaker. One voice ⇒
plain "Remote". The window colors speakers 2–6 apart.
**Names from Teams** (`speaking.rs`): Teams rings whoever speaks, and accessibility shows that
only as a class toggling on the tile / participants-list avatar (no "speaking" text; seen in
the AX tree's AXDOMClassList — UIA ClassName on Windows). During the call `ax::participants`
(same walk as `ui.jsonl`, every 500 ms) reads the video tiles (menu items labelled
"<Name>, …"; "Myself video, <Name>, …" = the user) and the participants list
(`calling-roster-item-<id>`; nothing inside a dialog: the share panel's "Browse OneDrive" is a
menu item too), each with all classes of its subtree (tiles: + 3 ancestors) and
its label's state words → `<call>/speaking.jsonl` (per participant `{t, k, n, me?, add, del}`,
changes only). The class names are generated and change between Teams versions, so after the
call the marker is learned per call, separately for tiles and list rows: of the tokens that
toggled, the one (and UI delay 0–1.5 s) whose lit time best matches the user's own mic speech
on their element and the remote track's speech on everyone else's (F1 ≥ 0.45). Remote words
then go to whoever holds the floor (±0.15 s; two about equal ⇒ abstain). The floor (`speaking::floor`): the
holder keeps it while lit; the previous holder lit again within 2.5 s gets it back; else the
most recently lit; suspect tiles (raised hand — an `id:` with "raisehand" or the label; lit ≥ 90 %
of the last 30 s; lit as much in silence as in speech) take a free floor only when nobody else
is lit. Element ids are recorded as `id:` tokens for the hand check and never become the marker. With the speaker model: a
voice takes the name ≥ 60 % of its named words carry (≥ 3 words, named ≥ 25 % of its words;
several voices on one name keep it: the model over-splits one person far more often than a room shares a tile); without it, segments are split by the lit name
(words < 0.7 s apart continue the previous one). Manual names (`speakers.json`) win; the
process-call skill (step 4a) names only the leftover "Speaker N", or the user runs
`echoctl speakers <call> 2="Name"` (segments + transcript.md re-rendered). Dev:
`echoctl ax-speaking [--tree ui-tree-120s.json]` (live: tokens flipping per participant),
`echoctl speaking <call>` (the marker learned and who it lights). **The marker** (Teams
2026-10): the speaker's video tile (the `AXMenuItem` "<Name>, …") gets a direct child with the
class `vdi-frame-occlusion` — the ring; language-independent, no participants pane needed.
With two or more others, calibration only takes a class that lights one tile at a time (`exclusivity` ≥ 0.6:
the ring measured 0.88–0.96; layout classes, "video is on" and the ring's opposite 0–0.36 — on 2026-10-29 they
out-fitted the ring and named everyone after one person), and the ring wins whenever it passes that (F1 ≥ 0.3).
`echoctl speakers <call> --relabel` names a call's voices again from its `speaking.jsonl` (manual names stay).
Confirmed in recorded trees: at each snapshot exactly one tile had it, the person speaking
then (and the participants list's avatar flipped class at the same moments). It is
`KNOWN_MARKERS` in `speaking.rs`: chosen when it fits (calibration still learns the delay,
and finds a successor class if Teams renames it). Teams' 2-person compact view shows no ring
(live, 2026-10-13: only the call timer changed while both talked) — covered by the rule below.
So also: **one other person in the call** (`speaking::sole_remote`: exactly one remote video
tile, or one participants-list row that is muted/unmuted — invitees not in the call are listed
as "No response") ⇒ the whole remote track is them, before any splitting, whatever the invite
says.
Per track (mic = Me, remote = Remote) → energy VAD into ≤25 s utterances → language detected per
utterance, restricted to `config.languages` (a new data folder starts with en, uk, ru, es; `[]` =
any; stops uk↔ru confusion), using **turbo as the
language-ID model** when downloaded (large-v3's detection is indecisive on accented English:
`uk 0.41/ru 0.27/en 0.25`, and forcing the wrong language makes Whisper *translate*; turbo said
en 0.99). Uncertain detections keep the track's previous language. Do NOT pick languages by
decode log-prob: translations/hallucinations score higher than faithful text. → whisper beam search with
that language pinned, `no_context` → drop silence (no-speech > 0.6, or > 0.2 with mean token log-prob < −1: Whisper's rule), bare
word lists ("Varkin, Node, Cage, Fork": `is_word_list`) and known hallucinations
(incl. Russian subtitle credits, `*Song*`-style annotations, lone fillers) → drop mic segments
that echo remote text within ±4 s (user on speakers) → merge by time into transcript.md.
Vocabulary hint (people + meeting terms + this call's vocabulary; ≤ 30 terms ≤ 50 chars) is a bare
comma list. The vocabulary (`vocab.rs`): `config.vocabulary` = the user's always-on terms
(`vocab add --always`); `config.learned` = terms the notes run learned (`vocab add --call <dir>`),
each with the meeting subjects (normalised: no digits/punctuation) and other people (email) it
came up with, used again only when a call shares a subject, has mostly those people (≥ half of
its other people), or its invite names the term. One list for all calls boosted one project's
terms into every other call (2026-10-13); the old list was migrated once (in `Config::load`):
each term scoped to the calls whose **notes** mention it (not transcripts — they can hold the
insertions), unmatched ones only by invite.
Final pass: every hinted utterance ≥ 1 s is also decoded without it, and the hinted text is kept
only within 0.6×…1.8×+80 chars of the plain one (`hint_is_sane`) and when hint words the plain decode lacks are under half of it (`hint_adds_terms`); live keeps the "≥ 3 s" rule.
Decode: beam 5, temperature 0, suppress non-speech tokens, token timestamps; VAD 800 ms silence
cut, ~250 ms pad, ≥ 200 ms speech. Ordering: Whisper's segment starts are
only ~1 s accurate, so segments are split into sentences timed by their own tokens
(`split_sentences`), and `order_turns` puts a reply after the line it overlaps when both start
within 0.6 s and the reply ends later.
Echo: offline AEC3 (`aec.rs`, pure-Rust port) on a copy of the mic track against the remote
track, only when their loudness envelopes correlate (speakers, not headphones); `config.echo_cancel`.

## Checks
- `cargo clippy`, `cargo test` (macOS) and `cargo check --target x86_64-pc-windows-msvc --no-default-features`
  must all be clean. (`--no-default-features` drops whisper.cpp, which can't cross-compile C++ to MSVC.)
- That cross-check also drops the `brain` feature (bundled SQLite is C), so Windows-only code behind it is never
  compiled there: v0.4.1-beta's Windows build broke on one such line. On Windows, before a release:
  `cargo check -p echoctl --no-default-features --features tray,brain` (native MSVC, no cmake needed) must be clean;
  `scripts/package-windows.ps1` then builds everything (`transcribe` + `vulkan`).
- Mod: `scripts/check-mod.sh` (plugin validate + `claude plugin test` + the window's node tests,
  `crates/echoctl/ui/notes-sync.test.mjs` (`NotesSync`) and `calendar-dates.test.mjs` (the calendar's days, hours
  and lanes) + tsc against the engine's types).
  Mod patterns used here: band button,
  `command.run` returning `{}` (no context), `ui.open({columns: 400})`, sizing from
  `e.props.bodyColumns` / `e.props.scroll.bodyRows`, `$.ui.mount` tests.
