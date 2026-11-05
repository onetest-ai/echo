# Recording and transcription — design

How Echo notices a Teams call, records it as two aligned tracks (or a memo as one), captures what Teams shows around
it, and turns the audio into a transcript on-device or on a server the user picked.

Related: rules in `.ai-run/guides/capture/capture-and-transcription.md` (arm / record / stop, tracks, transcription
guards — read it first; this doc does not repeat it). Windows status: `docs/windows-todo.md`. Live notes are the AI
area's (`docs/live-notes-design.md`, `.ai-run/guides/design/ai-and-minutes.md`).

## Modules

| File | Role |
|---|---|
| `watch.rs` | The watcher loop: agenda offers, call presence, arm → record → stop, back-to-back split, recovery, chat capture, ad-hoc people |
| `recorder.rs` | `Recording` (`start`, `start_memo`, `poll`, `finish`); call folder naming |
| `audio/mod.rs` | `Track` supervision, writer (resample to 16 kHz, gap padding), `repair_wav` |
| `audio/mic.rs` | Mic source; `MUTED` makes it record silence |
| `audio/macos.rs` | Core Audio process tap on Teams + tap-only aggregate device |
| `audio/windows.rs` | WASAPI output-device loopback, call detection from capture sessions |
| `ax/{mod,macos,windows}.rs` | Teams UI via Accessibility / UI Automation: `CallWindow`, `MuteWatch`, `UiRecorder`, `Sharing`, `participants`, `call_title`, `call_elapsed`, `call_glance` |
| `permissions/{macos,windows}.rs` | TCC preflight and the tap probe |
| `frames.rs` | Screen frames of the call window while someone else shares |
| `memo.rs` | Memo parts by thread (`parts`, `parts_for`, `whole_for`, `set_threads`) |
| `live.rs` | The in-call transcription worker → `live.jsonl`; `live-replay` |
| `transcribe.rs` | `Engine` (Whisper / Nemotron), `Transcriber` (local or server), filters, hint guards, ordering, `write_transcript` |
| `nemo.rs` | NeMo-Speech.cpp C ABI loaded at runtime |
| `remote.rs` | Transcription server and clients (Echo protocol, OpenAI-compatible) |
| `speakers.rs` | Diarization (Sortformer), voice → name labelling, `speakers.json` |
| `speaking.rs` | Who Teams lights: `speaking.jsonl`, marker calibration, `floor`, `sole_remote` |
| `aec.rs` | Offline AEC3 port; mic/remote envelope correlation |
| `vocab.rs` | The vocabulary hint for a call (`for_call`) |
| `gpu.rs` | One-time GPU check in a child process (`gpu-check`), `status`, `retest` |
| `graph.rs` | `chat_named`, `find_call_chat` (ad-hoc people and chat) |

## How it works

### Capture per platform
- **macOS 14.2+**: a Core Audio process tap on every `com.microsoft.teams2*` process, inside a private **tap-only**
  aggregate device (no output sub-device). Tap-only = clocked by the tap at a fixed 48 kHz and immune to output
  switches; with the output device as clock it runs at the headset's rate (24 kHz for AirPods in call mode). In a call
  `com.microsoft.teams2.modulehost` shows input + output running.
- **Windows 10 2004+**: remote track = WASAPI loopback of the **output device** Teams plays on (follows device
  switches; never while Teams isn't running). New Teams is blind to process loopback (exact zeros), so other sounds on
  that device are recorded too — README and `doctor` say so. `ECHOCTL_LOOPBACK=process` / `ECHOCTL_LOOPBACK_PID` for
  diagnostics; `echoctl procs` lists capture and playback sessions.
- **Call presence**: the app holds the mic (2 s release debounce, `MIC_RELEASE_DEBOUNCE`). Windows: an active capture
  session in Teams' process tree; its endpoint is the mic track's device. Per-tick queries are shared (process tree
  snapshot 2 s, mic sessions 0.9 s) between detection and both tracks.
- **TCC (macOS)** attaches to the responsible app: Echo.app when the app records, the terminal when the CLI captures
  by hand — so `doctor` asks the running app (`doctor.request`). Preflight returns "unknown" even when granted and a
  denied tap delivers zeros; the only reliable check is `doctor --probe` (1.5 s global tap, peak > 0). The first tap
  started triggers the prompt. `ECHOCTL_DEBUG=1` prints raw values.

### Teams' UI (`ax`)
- `ax::CallWindow` = the window holding `microphone-button` (AXDOMIdentifier on macOS, AutomationId on Windows — the
  same web UI in WebView2); never the main window with chats. Found once (~60 ms), re-checked every 10 s.
- Windows: Teams windows via `EnumWindows` (UIA's desktop children wait seconds on hung windows), one cached subtree
  call per window; WebView2 builds its tree only after the first request.
- **Mute sync** (`MuteWatch`, `config.follow_app_mute`, default on): the mic button's label every 400 ms;
  "Unmute…" ⇒ muted ⇒ `audio::mic::MUTED` records silence; events `mic_muted` / `mic_unmuted`. `echoctl ax-mic`.
- **`ui.jsonl`** (`UiRecorder`, `config.capture_ui`, default on): call-window text diffs every 1 s on the audio clock
  plus `ui-tree-{10,120,600}s.json` snapshots — raw material for names, timers, titles.

### Track supervision
`audio::Track` supervises one `Source`, polls `health()` every 250 ms and reopens on device / rate / process-set
change or stall (contract in the guide). Restarts back off 1 s → 30 s, slow after 20, count reset after 5 min stable.
A track of exact digital zeros for 5 min is reopened (≤ 10 times; also what a revoked TCC grant looks like). A writer
failure (full disk, volume gone) → track event `write_failed`, event `track_failed`, one notification. Restarts are
logged as `events` in `meta.json`.

### Arm, record, stop (`watch::run_loop`)
1. Agenda (`.echo/agenda.json`) → `meeting_soon` from T−58 s to T+4 min; `config.scheduled` / `unscheduled` = `ask`
   shows the app's own "Record?" dialog (`app::yes_no`; clients never ask), `manual` = only explicit arms.
2. A recording starts only with an arm (`armed.json`: written by `arm` / `join` / a scheduled meeting, single-use,
   `ARM_EXPIRES` 15 min) and a call present; it takes the arm (guide R1–R2).
3. Stop: `stop.request`, or `REJOIN_GRACE` (10 s) after the call ended. `echoctl stop` with nothing recording cancels
   the pending arm. User Stop mid-call → `held_arm` (guide R3).
4. Sleep (wall clock outruns the monotonic clock by > 30 s) closes the recording.
5. Everything is an event in `.echo/events.jsonl`; clients follow it (the mod, `events --follow`).
6. Languages can be pinned per call (`arm --languages`, Record menus; `MeetingContext.languages`) →
   `meta.json.languages` overrides `config.languages` live and final.

### Calls back to back
While recording, every 5 s (`CALL_LOOK_EVERY`) the watcher reads which call Teams is in (`call_seen`: the call
window's timer → when the call began, counted from the call's start so a rejoin reads the same; plus its name; one
`ax::call_glance` off the watcher's thread, `CallLook`).
- Began more than 90 s apart → another call. No timer: a different name counts only within 30 s after the mic came
  back (titles change within a call). Unreadable → same call; never split on doubt (`another_call`).
- A split needs the other reading confirmed on looks ≥ 9 s apart (`judge`): a real timer's start stays put, a
  clock-like text (chat time, countdown) drifts, two windows flipping never agree. A start first read well after the
  recording began is never taken as its own.
- Split: the recording ends (`next_call`, plus `call_ended` for the chat matcher); the ongoing call is new — a
  scheduled meeting is armed at once, a waiting arm starts it, else it is offered (once). `echoctl call-seen` prints
  what it reads.

### Memos
The user alone, recorded by hand (Record ▸ Memo, menu bar, ⌘K, `echoctl memo start` → `memo.request`). `meta.json`
`kind: memo`; mic track only, default input; no tap, mute sync, UI capture, frames or chat. One recording at a time
(guide R4); only Stop ends it, and the pill's Stop doesn't ask. Transcript in ~10 s paragraphs (`render_markdown`'s
`turn_cap`) so times mean something. Minutes come at once, for any memo with words (`calls::memo_gate`), with
`crates/echoctl/templates/memo.md`.
**Threads come from what was said**, not a picker (one memo can cover several threads):
1. `context` gives the run `memo` and `open_threads`; process-call step 4b splits the memo into parts by thread.
2. `echoctl memo threads <call> --file memo-threads.new.json` (`memo::set_threads`): unknown, closed and excluded
   threads dropped (named by id and reason, never echoed), spans clamped; the memo is pinned to each thread, earlier
   automatic pins not named again are taken back (never the user's); saved to `<call>/memo-threads.json`. Checks and
   pins run under the threads' lock, so a "Not this" clicked meanwhile wins.
3. A thread's sync reads only its part (`ledger::call_item(dir, Some(thread))`, `memo::parts_for`); a memo pinned by
   hand gives the whole; one reached only through rules gives nothing (`memo::whole_for`). Recall scopes sections by
   part (`brain/store.rs::memo_section_refs`).
The window shows each part as a chip with its times; × = Not this.

### Per-call extras
- **Frames** (`frames.rs`, `config.capture_frames`, default off; macOS needs Screen Recording + Accessibility): the call
  window every 2 s (macOS `screencapture -l` on its window id; Windows `PrintWindow(PW_RENDERFULLCONTENT)`), only
  while someone else shares (`ax::Sharing`: the stage's `popout-content-button`, any language; English "Content shared
  by <name>" → `shared_by`). Nothing while you present (Teams doesn't say which window). Kept when settled and new
  against **every** kept frame (128×72 thumbnail, ≥ 4 % new, ≥ 4 s apart, ≤ 150). A call with frames always gets
  minutes. Process-call prunes to cited frames (`echoctl frames <call> --keep …|none`); the rest **move** to
  `<home>/.echo/trash/<call>.frames/` with their index lines (purged after 30 days). Restore is manual.
- **Ad-hoc people** (`watch::find_call_people`, only when no `meeting.json`): call-window title (`ax::call_title`,
  retried ~1 min) → `ax::call_name` (first " | " part, view name stripped) → `graph::chat_named` (group chat with that
  topic, or 1:1 with that person) → `meeting.json` (`source: teams-call`) + `chat.id`. No sign-in: the title alone if
  it looks like a name. These names miss the in-call hint (built at start). `echoctl call-people`.
- **Chat** (`watch::start_chat_capture`, `config.capture_chat`, on with a Microsoft 365 sign-in): Graph `Chat.Read`.
  Chat id from the join link; ad-hoc → `graph::find_call_chat`: a recent chat whose "call started" precedes the
  recording with no "call ended" since, and within 3 min of when Teams' timer says the call began
  (`ax::call_elapsed`). Timer unreadable ⇒ the call must have started after you last left one. Kept: 2 min before
  start to 2 min after end (recurring meetings share a chat); polled every 30 s, final read 60 s after the end.
  `echoctl calendar chat <call>` backfills from `ui-tree-10s.json` and `ui.jsonl`.

### Transcription flow
1. **During the call** (`live.rs`): a worker reads `mic.wav` / `remote.wav` as they grow — the WAVs are the queue,
   nothing is dropped when decoding lags (≤ 60 s buffered per track, 4 threads) — and decodes each finished utterance
   with `transcribe::final_transcriber` → `live.jsonl`.
2. **At stop**: the worker decodes the rest; `watch::finish_transcript` (holding `.transcribing`) runs
   `transcribe::write_transcript` (order, echo filter, names, audio deleted) — seconds after the call.
3. **Echo cancellation** (`aec.rs`): only when mic and remote loudness envelopes correlate (speakers, not headphones);
   then the mic copy is cancelled and decoded again. `config.echo_cancel`.
4. Any failure, no model at start, crash recovery and `echoctl transcribe` run the **full pass** over the WAVs with the
   same steps.

Per utterance: energy VAD (800 ms silence cut, ~250 ms pad, ≥ 200 ms speech, ≤ 25 s) → language restricted to the
allowed list (a new data folder starts with en, uk, ru, es; `[]` = any), **turbo as language-ID model** when
downloaded → decode pinned (beam 5, temperature 0, non-speech suppressed, token timestamps, `no_context`) → drop
silence (Whisper's no-speech rule), word lists, hallucinations → drop mic segments echoing remote text within ±4 s →
sentences timed by their own tokens (`split_sentences`; Whisper segment starts are only ~1 s accurate) →
`order_turns` (a reply goes after the line it overlaps when both start within 0.6 s and it ends later).

**Vocabulary hint** (`vocab::for_call` + people + meeting terms, `build_prompt`): always-on terms, the call's threads'
terms, and learned terms scoped to meetings (subject, people, invite) — one global list once boosted one project's
terms into every other call. Learned terms now live in the Recall taxonomy (`.ai-run/guides/design/recall.md`). Guards against the
hint: guide R8–R10.

### Engines
Choosing a model chooses its engine (`models::Engine` per catalog entry; guide R5).
- **Whisper** (whisper.cpp, linked in). GPU: Metal on macOS; on Windows the GPU is tried once per version in a child
  (`echoctl gpu-check`, at app start) → `<config dir>/gpu.json`; failed or died ⇒ CPU. `config.use_gpu` overrides;
  Health shows "Speech engine: …" (`gpu::status`, never runs the check) and Re-test GPU (`gpu::retest`).
- **Nemotron** (`nemo.rs`, `nemotron-3.5-0.6b`, NVIDIA RNN-T, 742 MB GGUF): NeMo-Speech.cpp loaded at runtime (guide
  R6; `scripts/fetch-nemo-speech.sh` pins it by SHA-256; macOS copy in `Echo.app/Contents/Frameworks/nemo-speech`,
  signed by `package-macos.sh`). A build without it hides the model. One offline `recognize` per utterance. Language:
  one allowed ⇒ pinned; else an `auto` decode of the first 4 s, restricted + sticky, uk vs ru settled by the letters it
  wrote (і ї є ґ / ы э ё ъ — its own tags mislabel Ukrainian), then decoded with that locale pinned. Vocabulary =
  boosted phrases at 0.5, kept only through `boost_is_heard`. It never translates and doesn't invent text in silence.

### Transcription server (`remote.rs`)
For laptops without a usable GPU. VAD, filters, AEC and the transcript stay on the client; only utterances travel.
- **Echo server**: any Echo with Settings → Transcribe for other computers (`echoctl server share on` →
  `transcription-serve.json` {on, port 47800, key}; `remote::run_in_app` follows it every 2 s). Plain-HTTP std listener:
  ≤ 8 connections, one utterance at a time, its active model loaded on first request and dropped after 10 min idle,
  wrong key answered after 1 s. Protocol: `GET /v1/hello`, `POST /v1/utterance` (u32 LE JSON length +
  `{languages, prev, prompt}` + PCM s16le) → `{lang_id, lang, segs}`. Key: 16 chars, no look-alikes, compared without
  case or dashes.
- **Client** (`remote::Client`, `http://` or `https://` via ureq + rustls with the OS store; URL + key in
  `transcription-server.json`): server down ⇒ local model for 60 s, then retry; no local model ⇒ wait ≤ 15 min (the
  WAVs hold the audio); a refusal is final (guide R13).
- **OpenAI-compatible** (`ClientSettings.api: openai` + `model`; `echoctl server use <url> --openai`): connecting lists
  `/v1/models` and sends a test tone (`words` = word timestamps came back). Each utterance a WAV to
  `/v1/audio/transcriptions` (`verbose_json`); language pinned when the call has one, else the server's detection
  checked against the list (outside → asked again pinned to the track's last language). Hint as `prompt` + plain decode
  + `choose_hinted`.

### Speakers and names
1. **Diarization** (`speakers.rs`, `speakers-nemotron-3`, Sortformer ≤ 8 speakers, same runtime; on whenever
   downloaded): unless a 1:1 invite names the remote side, the remote track is diarized once (`v3-offline`, fed in 10 s
   pieces); each segment gets the speaker overlapping it most → "Speaker N". A shared sentence is split by word times
   (in memory only), corrected per engine first (Whisper +0.3 s, Nemotron −0.4 s — without it each turn's first words
   went to the previous speaker). One voice ⇒ "Remote".
2. **Who Teams lights** (`speaking.rs`): Teams marks the speaker only with a class toggling on the tile / roster avatar.
   `ax::participants` every 500 ms reads tiles ("<Name>, …"; "Myself video, …" = the user) and roster rows
   (`calling-roster-item-<id>`, nothing inside a dialog), with their subtree classes → `speaking.jsonl` (changes only).
3. **Calibration** after the call, per call, separately for tiles and rows: the toggling token and UI delay (0–1.5 s)
   whose lit time best matches the user's mic speech on their element and remote speech on others' (F1 ≥ 0.45).
   `KNOWN_MARKERS` (`vdi-frame-occlusion`, the ring) wins when it fits (F1 ≥ 0.3); with two or more others only an
   exclusive class counts (guide R15) — layout and "video on" classes once out-fitted the ring and named everyone after
   one person. Element ids are `id:` tokens for the hand check, never the marker.
4. **Floor** (`speaking::floor`): the holder keeps it while lit; the previous holder lit again within 2.5 s gets it
   back; else the most recently lit; suspect tiles (raised hand, lit ≥ 90 % of the last 30 s, lit as much in silence as
   in speech) take a free floor only. Remote words go to the floor holder (±0.15 s; near tie ⇒ abstain).
5. **Naming**: with diarization a voice takes the name ≥ 60 % of its named words carry (≥ 3 words, ≥ 25 % named;
   several voices may share a name — the model over-splits far more often than people share a tile); without it,
   segments split by lit name (words < 0.7 s apart continue). **One other person** (`sole_remote`: one remote tile, or
   one roster row that is muted/unmuted) ⇒ the whole remote track is them — Teams' 2-person compact view shows no ring.
   Precedence: guide R14. Leftover "Speaker N" are named by process-call step 4a or `echoctl speakers <call> 2="Name"`;
   `--relabel` re-runs naming from `speaking.jsonl`. Dev: `echoctl ax-speaking`, `echoctl speaking <call>`.

### Recovery (`watch::recover`, at app start, one full pass at a time)
Unclosed recordings get WAV headers fixed (`audio::repair_wav`) and are transcribed; a marker whose process still lives
(a transcription child of the previous app) is watched until it exits, then transcribed if it left nothing. Failures
leave `transcribe.error` and are not retried (guide Transcription R3).

## Decisions

| Decision | Why | Where |
|---|---|---|
| Tap-only aggregate device, no output sub-device | Fixed 48 kHz and immune to output switches; output-clocked ran at 24 kHz on AirPods | `audio/macos.rs` |
| Windows remote = output-device loopback, not process loopback | New Teams gives exact zeros to process loopback | `audio/windows.rs` |
| `doctor` checks permissions inside the running app; `--probe` taps for 1.5 s | TCC binds to the responsible app; preflight lies and a denied tap delivers zeros | `app.rs` (`doctor.request`), `permissions/macos.rs::probe_peak` |
| Reopen a track after 5 min of exact zeros (≤ 10) | Dead stream or revoked TCC looks like zeros | `audio/mod.rs::SILENT_RESTART_AFTER` |
| Read only the call window, never the main one | The main window shows chats; UI text, frames and mute come from the call | `ax::CallWindow` |
| Split a recording only on a confirmed other call; doubt = same call | Back-to-back calls ran into one recording; false splits are worse | `watch.rs::another_call`, `judge`, `CONFIRM_AFTER` |
| Frames only while someone else shares, new vs every kept frame | Your own share's window is unknown; repeated slides differ ~1–2 % | `frames.rs::NEW_CONTENT`, `MAX_FRAMES` |
| Pruned frames are moved to trash, not deleted | A model chose what to drop | `frames.rs` |
| Ad-hoc chat must match Teams' call-start time within 3 min | An old meeting that never posted "call ended" was taken for a 1:1 call | `graph::find_call_chat`, `ax::call_elapsed` |
| Live worker uses the final transcriber; WAVs are the queue | One pass: transcript seconds after the call, nothing dropped when decoding lags | `live.rs`, `transcribe::final_transcriber` |
| AEC only when loudness envelopes correlate | On headphones there is no echo and nothing to redo | `aec.rs::ECHO_CORRELATION` |
| Turbo as language-ID model | large-v3 is indecisive on accented English, and a wrong pinned language makes Whisper translate | `transcribe.rs` |
| Nemotron uk/ru by letters written | Its own tags mislabel Ukrainian | `transcribe.rs` (`Nemo`) |
| Nemotron boost 0.5, guarded by `boost_is_heard` | 1 garbles, 2–3 insert terms; unguarded it wrote product names dozens of times into an unrelated call | `transcribe.rs::boost_is_heard` |
| Learned vocabulary scoped to meetings | One global list boosted one project's terms into every call | `vocab.rs::for_call` |
| Windows GPU tested once per version in a child | A Vulkan driver crashed the process inside whisper.cpp | `gpu.rs` |
| Windows whisper.cpp: AVX2 not native, `/O2`, `+crt-static` | Native crashed on CPUs without AVX-512; the cmake crate drops /O2 (~10× slower); no VC++ redistributable | `packaging/windows/whisper.cmake` |
| Server is plain HTTP | The user's choice for a local network; https works through a proxy | `remote.rs` |
| Word times corrected per engine before diarization labels | Without it each turn's first words went to the previous speaker | `speakers.rs` |
| Speaking marker learned per call, `KNOWN_MARKERS` preferred, exclusivity required | Teams' class names are generated and change between versions | `speaking.rs::calibrate` |
| One remote person ⇒ whole remote track is them | Teams' 2-person view shows no ring | `speaking::sole_remote` |
| Call folders never reused (`-2` suffix) | Two recordings in the same second would truncate each other | `recorder.rs` |
| One app per user via an OS lock | The heartbeat alone can't prevent two watchers | `app.rs` (`app.lock`) |

## Files and state

| Path | What | Written by |
|---|---|---|
| `<home>/.echo/armed.json`, `armed.lock` | The pending arm (id, single-use) and its lock | `arm` / `join` / schedule; taken by the watcher |
| `<home>/.echo/stop.request`, `memo.request` | Control requests | CLI, window, tray |
| `<home>/.echo/events.jsonl` | Event stream (≤ 5 MB) | watcher |
| `<call>/meta.json` | Call facts: kind, languages, track events, end, `stop_reason` | `Recording`, under `.meta.lock` |
| `<call>/mic.wav`, `remote.wav` | 16 kHz mono tracks, until transcribed (`config.keep_audio`) | track writers |
| `<call>/live.jsonl` | In-call utterances | `live.rs` |
| `<call>/segments.json`, `transcript.md` | Final transcript | `transcribe::write_transcript` |
| `<call>/.transcribing`, `transcribe.error` | Marker with pid; failure note (not retried) | transcription |
| `<call>/ui.jsonl`, `ui-tree-{10,120,600}s.json` | Call-window text diffs and snapshots | `ax::UiRecorder` |
| `<call>/speaking.jsonl` | Who Teams lit, per participant | watcher (`ax::participants`) |
| `<call>/speakers.json` | Manual speaker names | `echoctl speakers`, process-call |
| `<call>/frames/*.jpg`, `frames.jsonl` | Shared-screen frames | `frames.rs` |
| `<home>/.echo/trash/<call>.frames/` | Pruned frames, 30 days | `echoctl frames --keep` |
| `<call>/meeting.json` (`source: teams-call`), `chat.id`, `chat.json` | Ad-hoc people; the meeting chat | watcher, `graph.rs` |
| `<call>/memo-threads.json` | A memo's parts by thread | `memo::set_threads` |
| `<config dir>/gpu.json` | GPU check result per version | `echoctl gpu-check` |
| `<config dir>/transcription-server.json` (0600), `transcription-serve.json` | Client URL + key; server share settings | `echoctl server` |

## Known gaps
- Speaker names, `shared_by` and the call timer depend on Teams' UI; Windows paths of these (dialogs skipped, tiles,
  timer text) are unconfirmed — see `docs/windows-todo.md`.
- Windows records every sound on Teams' output device, not Teams alone.
- Frames capture nothing while the user presents.
- Ad-hoc call names found after start miss the in-call vocabulary hint.
- OpenAI-compatible servers measured (Ollama + `lfm3-audio`) return no language and no word times, so uk/ru can't be
  restricted, one segment per request; clearly weaker than Whisper turbo on Ukrainian and terms.
- Restoring pruned frames is manual (move files back, append their lines to `frames.jsonl`).
