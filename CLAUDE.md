# Echo

A standalone app (Rust: tray icon + call watcher + CLI) that records Teams calls the user opts in
to (their mic + the Teams app's audio, as two tracks) and transcribes them on-device, plus a
Claude Code plugin that is its client (live pane, notes, follow-ups). **Separation of concerns:**
the app never depends on Claude Code (it asks with native dialogs, starts at login, has its own
permissions); the plugin drives it only through the CLI and the files in the data folder.
Notes/actions are written only by a Claude session (the app writes transcripts, never notes).

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
  `config` (incl. `--notes-in`), `transcribe`, `pending --for`, `claim --for`, …
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
  `app quit` before files are replaced, `app start` after; uninstall keeps data. Plus the same files as a zip.
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
  call come from the tray's "Add a note…" (`echoctl note` → `my-notes.md`).
  Note templates: `crates/echoctl/templates/*.md`, compiled in (user overrides in `<home>/.echo/templates/`).
  Post-call facts come from `echoctl context <call>` (gate: ≥ 3 min and ≥ 1,500 chars, or any saved frames; heading
  budget by length; colleagues vs external by email domain; previous notes of a recurring meeting).
- `<home>/calls/<ts>-<slug>/` — `meta.json`, `transcript.md`, `segments.json`, `notes.md` (git-ignored). `mic.wav`/`remote.wav`
  exist only until transcription succeeds, then are deleted (`config.keep_audio`, default false) — text only.
- `<home>/.echo/` — `config.json` (model, languages, vocabulary), control files
  (`armed.json`, `stop.request`, `quit.request`, `doctor.request` → `health.json`), `events.jsonl`,
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
WScript popup); clients don't ask.
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
with % / remove), Claude Code (connect = install plugin at user scope; where notes are written),
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
Calls (list + Notes / Transcript / Chat / Screens / Actions), Today (agenda + recap + Record /
Join), Record/Stop status. wry (Tauri's webview crate: WKWebView / WebView2) on the tray's tao
loop — not full Tauri (it would take over our loop, tray, packaging, updater). Content only via
the `echo://` protocol (`http://echo.localhost` on Windows): compiled-in `ui/index.html`,
`app.js`, `app.css` (vanilla, no npm/CDN; the octopus symbol is injected from
`assets/octopus.svg`), `/api/{state,calls,agenda,call/<name>}` JSON, `/file/<call>/frames/<img>`
after a path check; strict CSP; everything rendered is escaped (chat/transcripts are others'
text). Notes Markdown (`markdown()` in app.js, our own parser: headings, nested and task lists,
tables, quotes, rules, fences, inline code/bold/italic/strike, https links, `![](frames/x.jpg)` of
that call only, `[mm:ss]` anchors; text escaped before markup). A ```` ```mermaid ```` fence is drawn by
the vendored mermaid 11.4.1 (`ui/vendor/mermaid.min.js`, `/assets/mermaid.js`, ~2.5 MB, loaded on
first use, `securityLevel: 'strict'`, no eval, so the CSP holds), themed from the CSS tokens and
cached per source + theme so the 2 s refresh doesn't redraw it. A diagram that fails shows its
source and the error. Parser helpers are `md*`: app.js is one global scope (a `list()` there
is the calls list). Actions go back over IPC (`window::handle`: toggle action, delete, reveal, record, stop,
arm/join meeting, copy) through the same functions as the CLI. The page polls the local API
every 2 s and re-renders only when the data changed (keeps selection/scroll). Closing the
window keeps the app in the menu bar; macOS shows a Dock icon only while it is open. Notes need
Claude: a call without them shows the `/echo:process-call` command. Dev: `echoctl window
--selftest` (opens the real webview, page fetches via echo:// and reports over IPC),
`echoctl window --export <dir>` (the same responses as files, to review the UI in a browser).

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
starts the app if it is set up but not running, and catches up: `echoctl pending --for <cwd>`
(transcribed, no notes, ≥ 3 min, < 48 h; empty outside `config.notes_in`) → `echoctl claim
--for <cwd>` (atomic `.claimed`; one session per call) →
`/echo:process-call`. Band above the prompt while the app runs: `[ Live ]` (hotkey l) +
status. Pane: transcript left / summary-highlights-actions right (stacked under 100 cols). Every
1 s it heartbeats `<home>/.echo/mod.json`, tails `events.jsonl` (from the end, never
replays), and: on `recording_started` opens the `echo-live` pane; on `transcribed` claims
the call and submits `/echo:process-call <dir>` (unattended processing). The pane shows `<call>/live.jsonl` (written by `live.rs` during recording: the
call's one transcription pass, an utterance decoded after 0.8 s of silence — see Transcription) plus a rolling summary/highlights/actions from
`$.model.complete` (haiku, ≤ every 90 s, ≥ 400 new chars). `echoctl live-replay <wav> <dir>` (hidden) plays a WAV in as a growing recording (10×)
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

## Notes from the app (`notes.rs`)
Write notes (window Notes/Actions tabs, the pane's buttons, `echoctl notes <call>`) runs the same
`/echo:process-call` skill headless: `claude -p … --output-format stream-json`, through the
user's login shell (`$SHELL -l -i -c`, their env: gateways, `ANTHROPIC_*`, Bedrock/Vertex), cwd
= `notes_in` or the data folder, `--plugin-dir` = the plugin shipped in the app
(`Echo.app/Contents/Resources/plugin`; dev: repo `plugin/`; else the installed one), model =
`config.notes_model` (default sonnet; `default` = Claude Code's own; menu → Claude Code → Notes
model). `--permission-mode dontAsk` + an allowlist (Read/Glob/Grep in the data folder,
Edit/Write in `calls/`, `Bash(echoctl:*)`, the skill): transcripts are others' words and the
user's own default mode (auto / bypass) must not apply. `ECHOCTL_HEADLESS=1` inside: the mod
does nothing, `pending`/`claim` refuse. The CLI hands the run to the running app
(`notes-<call>.request`) unless `--wait`. State in `<call>/notes-run.json` (running / done /
skipped / failed / needs_login — "Not logged in · Please run /login" etc.), readable log in
`<call>/claude.log` (window → Show log). Failure releases `.claimed`. After transcription with
no live session (`mod.json` heartbeat < 10 s) the app notifies "Transcript ready — click Write
notes". Live pane model: `config.live_model` (default haiku).
A local notes model: the run's cwd (`notes_in`, else the data folder) gets a `.claude/settings.json`
whose `env` points Claude Code at Ollama's Anthropic API (`ANTHROPIC_BASE_URL`, `ANTHROPIC_AUTH_TOKEN`,
the default-model vars) and `notes_model` names the Ollama model (one with `num_ctx` ≥ 64K: Claude
Code's prompt + an hour's transcript). Claude Code warns `unrecognized_model` for any model it
doesn't know, so that counts as a failure only when the run failed (`model_rejected`).
Notes in other shapes (a user template with tables or YAML frontmatter): `calls::recap_from_notes`
skips frontmatter, reads a Decision table's cells and drops placeholders (`TBD`, `_None recorded._`);
the window shows a frontmatter as a small key/value table (`mdFrontmatter`).

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
  install mounts the dmg, requires our team id signature, swaps the bundle after exit. Windows:
  the release's MSI (offered only to Echo in `%LOCALAPPDATA%\Programs\echo`), checked against
  the asset's `digest` from the API (required: the builds are unsigned), run by `msiexec /qn`
  (per-user, no elevation; the MSI quits Echo, replaces the files, starts it). Every platform
  checks the digest when the API gives one. Code signing would add a publisher check on top.
- `me.md` profile → `context.me_profile`; `echoctl compose` opens a mailto draft (never sends);
  `calls::recap` (deterministic "last time" bullets) in the Record dialog and Upcoming menu;
  tray "Add a note…" while recording; "Notes ready" notification.
- Notes bullets carry `[mm:ss]`; the pane turns them into jumps; `actions.json` items have
  `t` + `status` (todo/done, toggled in the pane). Live quick actions: What did I miss / Suggest
  a question (haiku over the last 5/10 min).

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
two voices on one name ⇒ "Name (Speaker N)"); without it, segments are split by the lit name
(words < 0.7 s apart continue the previous one). Manual names (`speakers.json`) win; the
process-call skill (step 4a) names only the leftover "Speaker N", or the user runs
`echoctl speakers <call> 2="Name"` (segments + transcript.md re-rendered). Dev:
`echoctl ax-speaking [--tree ui-tree-120s.json]` (live: tokens flipping per participant),
`echoctl speaking <call>` (the marker learned and who it lights). **The marker** (Teams
2026-10): the speaker's video tile (the `AXMenuItem` "<Name>, …") gets a direct child with the
class `vdi-frame-occlusion` — the ring; language-independent, no participants pane needed.
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
- Mod: `scripts/check-mod.sh` (plugin validate + `claude plugin test` + the window calendar's node test,
  `crates/echoctl/ui/calendar-dates.test.mjs` + tsc against the engine's types).
  Mod patterns used here: band button,
  `command.run` returning `{}` (no context), `ui.open({columns: 400})`, sizing from
  `e.props.bodyColumns` / `e.props.scroll.bodyRows`, `$.ui.mount` tests.
