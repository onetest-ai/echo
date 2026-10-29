# Echo

A standalone app (Rust: tray icon + call watcher + CLI) that records Teams calls the user opts in
to (their mic + the Teams app's audio, as two tracks) and transcribes them on-device, plus a
Claude Code plugin that is its client (live pane, notes, follow-ups). **Separation of concerns:**
the app never depends on Claude Code (it asks with native dialogs, starts at login, has its own
permissions); the plugin drives it only through the CLI and the files in the data folder.
The final notes/actions (`notes.md`, `actions.json`) are written only by a Claude Code run (the app writes
transcripts, never notes). One exception, by design: the app saves model-written **live drafts** during a call
(`live-notes.json`, `live-asks.jsonl`, `livenotes.rs`), which never feed the minutes.

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
  same files as a zip.
- Marketplace `echoctl` (`.claude-plugin/marketplace.json`): `claude plugin marketplace add
  onetest-ai/echo` + `claude plugin install echo@onetest --scope user|project|local`
  (what `echoctl claude connect` runs; `ECHOCTL_MARKETPLACE` overrides the source).
- CLI on PATH (`app install-cli`): macOS `/usr/local/bin/echoctl` → the app's binary (admin
  prompt via osascript); Windows: app folder appended to the user PATH.
- `plugin/` — the Claude Code plugin; ships no binary: `plugin/scripts/echoctl[.cmd]` finds the
  CLI (`$ECHOCTL_BIN` → `plugin/bin` dev build from `scripts/build-plugin.sh` → the app's own
  path in `<config dir>/app-path` (written at each app start: any install folder) → PATH →
  Echo.app / `%LOCALAPPDATA%\Programs\echo`). On Windows Claude Code's Bash is Git Bash, so
  the skills run the **sh** launcher there too (it handles MINGW paths via `cygpath`). Skills:
  `process-call` (notes → actions → vocabulary → follow-ups), `live` (the mod opens the pane),
  `record` (arm / stop from chat), `doctor` (`echoctl up` + fix what's missing: data folder,
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
  `config.graph_client_id`, default CodeMie's public client for the Contoso beta; tokens in
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
runtime; the Dock icon shows only while the Echo window is open. Notifications (macOS) go
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
`ask` | `manual`; this user uses `manual` for both — nothing prompts, the user arms explicitly.
`scripts/build-plugin.sh` must replace `plugin/bin/echoctl` by rename, never overwrite in place:
macOS SIGKILLs (exit 137) a binary rewritten in place.

## The Echo window (`window.rs`, `ui/`)
Menu → Open Echo… (or `echoctl app open [--call <name>]`, or a recent call) opens one window:
Calls (list + Live / Minutes / Transcript / Chat / Screens / Actions, and **My notes** in the right panel), Calendar.
**A workbench** (`docs/plugins-design.md`): fixed areas — a 40 px top strip with the **Record pill** at its centre
(idle Record ▾ with language + profile, armed + Cancel, meeting soon + Record it, recording: clock, title, Stop that
asks first; "Writing the transcript…" beside it), the activity bar (`ACTIVITIES`: Calls `list-detail`, Calendar
`single`, and at its foot **Settings** — ⌘/Ctrl+, — `single`; ⌘/Ctrl+1…9), the primary view, right and bottom panels (`PANEL_VIEWS`, none yet: drawn only when a view is
in them; the first is **My notes**, right by default, open unless closed — in a narrow window only when asked
for, over the page), a status bar. The call header's actions are one ⋯ menu (folder, minutes page, delete) plus the
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
`assets/octopus.svg`), `/api/{state,calls,agenda,call/<name>}` JSON, `/file/<call>/frames/<img>`
after a path check; strict CSP; everything rendered is escaped (chat/transcripts are others'
text). **Settings** (activity, `#settings/<section>`): General, Recording, Transcription, AI & minutes, Calendar & chat, Vocabulary,
About me (name, email, and `me.md` — the profile the minutes read — shown rendered by `minutes::markdown_html`, the
minutes' safe writer, line by line, and edited in place: `settings_set me_profile`), Layout, Health (a ⚠ badge counts
required permissions missing). Vocabulary lists the learned terms with the meetings and people they are scoped to
(`learned_always` / `learned_remove`). `/api/settings` = one snapshot
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
panel moves keep it; keys inside `.xterm` all go to it (⌘/Ctrl+Esc gives focus back). Two panel views (bottom by
default): **Claude · this call** — guarded on the call's own profile (`ai::for_call`; a sign-in problem comes back
with its profile → Sign in…): the minutes run's guard with `ECHOCTL_RUN_MODE=ask` (Read/Glob/Grep in the call and the
previous minutes, Bash only `echoctl context` — `guard::ASK_COMMANDS` —, no Edit/Write/Skill), `--tools
Bash,Read,Glob,Grep --strict-mcp-config --permission-mode default`, allow rules for reading the call, headless,
Claude Code ≥ `guard::MIN_CLAUDE`; one call session at a time — and **Claude · Echo folder**, the user's own Claude
Code (not headless) on the profile they pick (`ai::ready`, default = the default profile). Both run in the data
folder (never a call's: Windows renames), hold their profile's lease while they run, start with `--session-id` and
resume with `--resume` (ids per profile in `<home>/.echo/sessions.json`: `<profile>:folder`, `<profile>:<stamp>`; one
refused within 4 s is forgotten). Entry points: status bar Claude, ⌘/Ctrl+`, the call's ⋯ → Ask Claude about this
call. `ai::terminal_command` = `claude_command`'s command and environment for a PTY (`ECHOCTL_DEBUG` prints its
arguments). `echoctl window --selftest` runs a shell in a terminal end to end (`terminal: true`).
**My notes** (a panel view, `notesPanel`; it was a call tab) = the user's own notes as a **timeline of moments**
(`NoteDoc`, no DOM: `ui/my-notes.test.mjs`): each note a line of `my-notes.md` — `- [12:05] …`, `? ` question, `! `
decision, `⚑` flag, `- [ ] [12:22] …` to-do, `  > "…"` the words it is about — kept in time order; a box adds one,
timed from its first key while the call records; select words in a transcript → Comment, or right-click a transcript turn or a line of the minutes → Add note (the
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
Closing the window keeps the app in the menu bar; macOS shows a Dock icon only while it is open. Minutes need
Claude: a call without them shows when they come, or the `/echo:process-call` command. Dev: `echoctl window
--selftest` (opens the real webview, page fetches via echo:// and reports over IPC; a sandboxed test frame proves
its IPC posts are dropped; the close handshake answers with its number),
`echoctl window --export <dir>` (the same responses as files, each minutes page as `minutes/<call>/index.html`, to review the UI
in a browser through a static server).

Calendar (window → Calendar): Day (today) and a rolling Week (yesterday, today, 5 days
ahead) time grid from `/api/calendar?start=&days=` (Graph `calendar_view` for the range,
cached 2 min, served by the async `echo://` handler so Microsoft 365 never blocks the UI;
without a sign-in: today's agenda). Each event carries `recorded` (a call matched by meeting
id, or by subject within the event's time) and `scheduled`; calls matching no event come back
as `calls`. "Record when it starts" → `schedule.rs` (`<home>/.echo/scheduled.json`): the watcher
arms a scheduled meeting from 60 s before its start until its end; the entry is removed when
its recording starts (stopping never re-arms it), when that arm is cancelled, or after the end.

## Live view (mod) and automation
`plugin/hooks/register.tsx` is a Claude Code mod (state contract `plugin/types/index.d.ts`, pure helpers
`hooks/view.ts`). At session start it asks the CLI for the data folder (`echoctl app` → `home`),
starts the app if it is set up but not running, and catches up: `echoctl pending --for <cwd>` → `echoctl notes <call>
--auto --for <cwd>` — both refuse now (empty / `{requested: false, waiting, reason}`): the app writes the minutes itself
(`notes::auto_due`), or nobody does automatically. Band above the prompt while the app runs: `[ Live ]` (hotkey l) +
status. Pane: transcript left / summary-highlights-actions right (stacked under 100 cols). Every
1 s it heartbeats `<home>/.echo/mod.json`, tails `events.jsonl` (from the end, never
replays), and: on `recording_started` opens the `echo-live` pane; on `transcribed` asks for the minutes (`notes
--auto`, refused as above; `waiting` → the pane follows the call until they are written, a run ends, or a later answer
(at most once a minute; a CLI that didn't answer is asked again) stops saying `waiting`). Its action ticks go through `echoctl action <call> <id> done|todo`. The pane shows `<call>/live.jsonl` (written by `live.rs` during recording: the
call's one transcription pass, an utterance decoded after 0.8 s of silence — see Transcription) plus the app's live notes
(`live-notes.json` → summary/highlights/actions, `live-state.json`, answers in `live-asks.jsonl`); its quick buttons call
`echoctl live ask`. It **never calls a model itself** (that would use the session's account, not the call's profile);
live notes off → it says so. `echoctl live-replay <wav> <dir>` (hidden) plays a WAV in as a growing recording (10×)
and writes the transcript, as after a call.
Mute sync: while recording, the watcher polls Teams' microphone button via Accessibility / UI
Automation (`ax::MuteWatch`, label "Unmute…" ⇒ muted) every 400 ms; muted ⇒ `audio::mic::MUTED` makes the mic
track record silence; events `mic_muted`/`mic_unmuted` drive the band. `config.follow_app_mute`
(default on). `echoctl ax-mic` (hidden) shows what the detector sees. `/echo:live` is a skill
the mod intercepts in `command.run` (mods can't register names with a colon).
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
and `notes --wait`; minutes on them wait while any call records or is transcribed. `live-state.json` mirrors the state
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
The window: `auto` = `{due: now|at|off|before|not_ready|tried|gate|missing|error, at, reason}` (`missing`: due, but
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
does nothing, `pending`/`claim` refuse. A session's `claim` (by hand: `/echo:process-call`) fails only while Echo's run
of the call is on or another session's fresh claim holds a call without minutes; a claim left by a finished run (Echo's
keeps it once `done`) is taken over. The CLI hands the run to the running app
(`notes-<call>.request`) unless `--wait` (Ctrl-C there stops its Claude Code too: `notes::shutdown`). The run uses the call's AI profile (launcher, model, strictness: above).
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
- `me.md` profile → `context.me_profile`; `echoctl compose` opens a mailto draft (never sends);
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
- Mod: `scripts/check-mod.sh` (plugin validate + `claude plugin test` + the window's node tests,
  `crates/echoctl/ui/notes-sync.test.mjs` (`NotesSync`) and `calendar-dates.test.mjs` (the calendar's days, hours
  and lanes) + tsc against the engine's types).
  Mod patterns used here: band button,
  `command.run` returning `{}` (no context), `ui.open({columns: 400})`, sizing from
  `e.props.bodyColumns` / `e.props.scroll.bodyRows`, `$.ui.mount` tests.
