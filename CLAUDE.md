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
- Transcription is on-device (whisper.cpp via whisper-rs, `transcribe` feature).

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
  `scripts/package-windows.ps1` (zip; unsigned for now).
- Marketplace `echoctl` (`.claude-plugin/marketplace.json`): `claude plugin marketplace add
  onetest-ai/echo` + `claude plugin install echo@onetest --scope user|project|local`
  (what `echoctl claude connect` runs; `ECHOCTL_MARKETPLACE` overrides the source).
- CLI on PATH (`app install-cli`): macOS `/usr/local/bin/echoctl` → the app's binary (admin
  prompt via osascript); Windows: app folder appended to the user PATH.
- `plugin/` — the Claude Code plugin; ships no binary: `plugin/scripts/echoctl[.cmd]` finds the
  CLI (`$ECHOCTL_BIN` → `plugin/bin` dev build from `scripts/build-plugin.sh` → PATH →
  Echo.app). Skills:
  `process-call` (notes → actions → vocabulary → follow-ups), `live` (the mod opens the pane),
  `record` (arm / stop from chat), `doctor` (`echoctl up` + fix what's missing: data folder,
  permissions, model, languages; first run itself is the app's). The user's own notes during a
  call come from the tray's "Add a note…" (`echoctl note` → `my-notes.md`).
  Note templates: `crates/echoctl/templates/*.md`, compiled in (user overrides in `<home>/.echo/templates/`).
  Post-call facts come from `echoctl context <call>` (gate: ≥ 3 min and ≥ 2,500 chars; heading
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
- Windows 10 2004+: WASAPI process loopback on root `ms-teams.exe`, include-process-tree mode.

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
Windows can't detect calls yet (`app_in_call` → None): arm starts recording, `stop` ends it.
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
through UserNotifications as Echo.app (its icon; a click opens the window; asks permission at
first start); dev builds fall back to osascript, which macOS shows as Script Editor. Menu: status, Record, Stop,
recent calls, Settings (data folder, start at login, install CLI), Speech models (use / download
with % / remove), Claude Code (connect = install plugin at user scope; where notes are written),
Health (Ask for permissions),
Quit — clicks call the same `arm`/`stop` as the CLI, so every client sees the same events.
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
text). Actions go back over IPC (`window::handle`: toggle action, delete, reveal, record, stop,
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
the call and submits `/echo:process-call <dir>` (unattended processing). The pane shows `<call>/live.jsonl` (written by `live.rs` during recording: turbo,
utterance decoded after 0.8 s of silence) plus a rolling summary/highlights/actions from
`$.model.complete` (haiku, ≤ every 90 s, ≥ 400 new chars). `echoctl live-replay <wav> <dir>` (hidden) tests live transcription.
Mute sync (macOS): while recording, the watcher polls Teams' microphone button via Accessibility
(`ax::MuteWatch`, label "Unmute…" ⇒ muted) every 400 ms; muted ⇒ `audio::mic::MUTED` makes the mic
track record silence; events `mic_muted`/`mic_unmuted` drive the band. `config.follow_app_mute`
(default on). `echoctl ax-mic` (hidden) shows what the detector sees. `/echo:live` is a skill
the mod intercepts in `command.run` (mods can't register names with a colon).
A call can pin languages (`arm --languages uk`, tray Record ▸ Only …) → `meta.json.languages`
overrides the configured list in live and final transcription.

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

## Robustness, per-call extras, updates
- Recovery at app start (`watch::recover`): recordings never closed get their WAV headers fixed
  (`audio::repair_wav`) and are transcribed; `.transcribing` holds the pid, stale markers are
  removed; failures leave `transcribe.error` (not retried). One transcription at a time.
- Sleep = wall clock outrunning the monotonic clock (> 30 s) → the recording is closed.
- Track restarts back off 1 s → 30 s (slow after 20, reset after 5 min stable); the remote
  track reopens after 5 min of exact digital zeros (≤ 10×; also what revoked TCC looks like).
- `<call>/ui.jsonl` (`ax::UiRecorder`, `config.capture_ui`, default on): Teams window text diffs
  every 1 s on the audio clock + `ui-tree-{10,120,600}s.json` — raw material for speaker names
  (participant tiles read via AX; no screenshots).
- `<call>/frames/*.jpg` + `frames.jsonl` (`frames.rs`, `config.capture_frames`, default off,
  needs full Screen Recording): Teams window via `screencapture -l` every 3 s, kept when settled
  and ≥ 12 % of a 64×36 thumbnail changed (≥ 8 s apart, ≤ 150). `context.frames` → the notes'
  "On screen" section; process-call then prunes to the cited frames
  (`echoctl frames <call> --keep …|none`), so only useful ones stay.
- `<call>/chat.json` (`watch::start_chat_capture`, `config.capture_chat`, default on with a
  Microsoft 365 sign-in): the meeting's Teams chat via Graph `Chat.Read` — chat id parsed from
  the join link (`19:meeting_…@thread.v2`); ad-hoc calls (no link) → `graph::find_call_chat`:
  the recently active chat whose "call started" system event precedes the recording with no
  "call ended" in between (retried 5 min). Kept: messages from 2 min before the call started to
  2 min after it ended (recurring meetings share one chat) — polled every 30 s, final read 60 s
  after the end. `[{t, from, text, files}]`, `t` on the audio clock; `<call>/chat.id`.
  `context.chat` → the notes' "In the chat" section; pane: Chat section (live) + Chat tab (`h`).
  `echoctl calendar chat <call>` backfills; hidden `calendar call-events` lists call events.
- Invariants from the code review: one app per user (`flock`/share-mode lock on
  `<config dir>/app.lock`, not just the heartbeat); `Config::save` is atomic and nothing saves
  after a failed load; `transcribe` refuses a call still recording and takes `.transcribing`
  exclusively; call folders are never reused (`-2` suffix); release tags must be plain semver
  and reach the swap script only as `"$1"…` arguments; Windows updates open the release page
  (unsigned builds can't be verified); live transcription is bounded and never joined at stop.
- Updates (`update.rs`): GitHub release check at start + every 6 h (public API, else `gh`);
  install mounts the dmg, requires our team id signature, swaps the bundle after exit.
- `me.md` profile → `context.me_profile`; `echoctl compose` opens a mailto draft (never sends);
  `calls::recap` (deterministic "last time" bullets) in the Record dialog and Upcoming menu;
  tray "Add a note…" while recording; "Notes ready" notification.
- Notes bullets carry `[mm:ss]`; the pane turns them into jumps; `actions.json` items have
  `t` + `status` (todo/done, toggled in the pane). Live quick actions: What did I miss / Suggest
  a question (haiku over the last 5/10 min).

## Transcription
Per track (mic = Me, remote = Remote) → energy VAD into ≤25 s utterances → language detected per
utterance, restricted to `config.languages` (stops uk↔ru confusion), using **turbo as the
language-ID model** when downloaded (large-v3's detection is indecisive on accented English:
`uk 0.41/ru 0.27/en 0.25`, and forcing the wrong language makes Whisper *translate*; turbo said
en 0.99). Uncertain detections keep the track's previous language. Do NOT pick languages by
decode log-prob: translations/hallucinations score higher than faithful text. → whisper beam search with
that language pinned, `no_context` → drop high no-speech-prob segments and known hallucinations
(incl. Russian subtitle credits, `*Song*`-style annotations, lone fillers) → drop mic segments
that echo remote text within ±4 s (user on speakers) → merge by time into transcript.md.
Vocabulary hint (people + meeting terms + glossary; ≤ 30 terms ≤ 50 chars) is a bare comma list.
Final pass: every hinted utterance ≥ 1 s is also decoded without it, and the hinted text is kept
only within 0.6×…1.8×+80 chars of the plain one (`hint_is_sane`); live keeps the "≥ 3 s" rule.
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
- Mod: `scripts/check-mod.sh` (plugin validate + `claude plugin test` + tsc against the engine's types).
  Mod patterns used here: band button,
  `command.run` returning `{}` (no context), `ui.open({columns: 400})`, sizing from
  `e.props.bodyColumns` / `e.props.scroll.bodyRows`, `$.ui.mount` tests.
