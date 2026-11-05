# Capture and Transcription

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: recording state machine, capture tracks, transcription engines and guards, live notes

**Design**: `.ai-run/guides/design/recording.md` — how it works and why; the rules here win.

## Recording

### Rules
- R1. Write, cancel, expire or take an arm only while holding `watch::arm_lock` (`armed.lock`), and start a recording only after `take_arm(id)` returned true: it removes `armed.json` only while it still holds that id, so an arm replaced or cancelled since the tick read it never starts, and one written during the recording (the next meeting's) is left alone. Evidence: `crates/echoctl/src/watch.rs::take_arm`
- R2. Keep an arm single-use: `run_loop` takes it before `Recording::start`, a start that fails leaves it used up (one error and one notification, no retry each tick), an arm older than `ARM_EXPIRES` is taken as `arm_expired`, and a scheduled meeting is removed with `schedule::remove_id` once its recording starts so stopping it never re-arms it. Evidence: `crates/echoctl/src/watch.rs::run_loop`
- R3. A user Stop while the call goes on ends only the recording: `run_loop` puts the pending arm's id in `held_arm` (not startable while it names it) and sets `stopped_in_call` (no scheduled meeting is armed); keep both reset where `run_loop` resets them, on call end, sleep and a next-call split, and keep a memo out of this path. Evidence: `crates/echoctl/src/watch.rs::run_loop`
- R4. Keep one recording at a time: `memo.request` is refused while anything records; a memo (`Recording::start_memo`, `kind: memo`) has only the mic track on the default input (`MicSource { app: None }`), only `stop.request` ends it (the call-ended stop skips `r.memo`), and a Teams call starting during it emits `call_during_memo` and is not recorded. Evidence: `crates/echoctl/src/recorder.rs::Recording.start_memo`
- R5. End a recording only through `Recording::finish`: it halts every track before finishing any (they end at the same instant), finishes both even when one failed (repairing that WAV's header), closes the live worker without joining it and writes the end under `calls::lock_meta`; `watch::finish` transcribes what was saved even when it returns an error. Evidence: `crates/echoctl/src/recorder.rs::Recording.finish`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Join the live worker or decode on the watcher's thread | Close it in `Recording::finish` and join it in the thread `watch::finish_transcript` spawns | a slow decoder must not hold up the watcher, the tray or the heartbeat | `crates/echoctl/src/watch.rs::finish_transcript` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| changing arm, record or stop | `crates/echoctl/src/watch.rs::run_loop` | the arm, start, memo, next-call and stop steps in tick order | `crates/echoctl/src/watch.rs::a_recording_takes_only_its_own_arm` |

## Capture tracks

### Rules
- R1. Give every track of a recording the recording's one `t0` (`Recording::start` passes the same `Instant` to each `Track::start`): `write_track` places each chunk on that timeline, pads gaps over `GAP_TOLERANCE` with zeros and pads to the stop time, so `mic.wav` and `remote.wav` stay sample-aligned however often a source reopens. Evidence: `crates/echoctl/src/audio/mod.rs::write_track`
- R2. A capture `Source::open` sends mono `Chunk`s tagged with its `epoch` and native `rate` (the writer resamples to `TRACK_RATE` itself), and its `LiveStream::health` returns `Health::Reopen` for a planned change (device, rate, the app's processes: reopened at once, outside the backoff) and `Health::Restart` for a failure (backed off 1 s doubling to 30 s); `Track::poll` turns more than 3 reopens in 30 s into a restart. Evidence: `crates/echoctl/src/audio/mod.rs::Track.poll`

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding or changing a capture source | `crates/echoctl/src/audio/mic.rs::MicStream.health` | errors and stalls as `Restart`, device and rate changes as `Reopen` | `crates/echoctl/src/audio/mod.rs::planned_reopens_skip_the_backoff_failures_dont` |

## Transcription

### Rules
- R1. Delete `mic.wav` / `remote.wav` only in `transcribe::write_transcript`, after `segments.json` and `transcript.md` are written and only when neither its `keep_audio` argument nor `config.keep_audio` is set, then note it in `meta.json` through `calls::update_meta`; a `--model` comparison run and `live-replay` pass `keep_audio = true`. Evidence: `crates/echoctl/src/transcribe.rs::write_transcript`
- R2. Hold `transcribe::take_marker` (`.transcribing`, created exclusively with this pid; a dead pid's marker is taken over, a live one fails with "already being transcribed") for as long as you write a call's transcript, as `transcribe` and `watch::finish_transcript` do; `watch::transcribe_now` treats that failure as another process's work, not a failed transcription. Evidence: `crates/echoctl/src/transcribe.rs::take_marker`
- R3. A transcription failure writes `<call>/transcribe.error` and is not retried: `watch::recover` skips a call with it or with `transcript.md`, waits for a marker whose pid is alive before transcribing, removes stale markers, closes unfinished recordings of the last 7 days with `close_unfinished` (headers repaired, `stop_reason: recovered`) and skips recordings this app run started. Evidence: `crates/echoctl/src/watch.rs::recover`
- R4. Change how utterances are decoded in `transcribe::Transcriber` / `Engine`, not in `live.rs`: the live worker builds its transcriber with `final_transcriber` (same model, language list and hint guard as `echoctl transcribe`) and reads the growing WAVs as its queue; `live::start` returns `None` with no model and no server, and the call is then transcribed after it ends. Evidence: `crates/echoctl/src/live.rs::run`
- R5. Add a speech engine as a `transcribe::Engine` variant (with its `EngineState`) implementing `utterance(prev, allowed, prompt, chunk)` → (language id, code, `Decoded`), a `models::Engine` variant, `CATALOG` entries naming it and its arm in `models::runnable`; `Engine::load` picks it by `models::engine_of`, and the transcription server decodes through the same `Engine`. Evidence: `crates/echoctl/src/transcribe.rs::Engine`
- R6. Load a runtime-only speech library as `nemo.rs` does: `libloading` at first use from `candidates()` (`ECHOCTL_NEMO_DIR`, the app's own copy, `<config dir>/nemo-speech`), with `available()` false when it is missing, never a link-time dependency (its ggml must stay apart from whisper.cpp's). Evidence: `crates/echoctl/src/nemo.rs::load_api`
- R7. Restrict each utterance's language to `allowed_languages` (the call's `meta.json` languages replace `config.languages`) and pick it with `choose_language` from detection probabilities: the top language when confident, else the track's previous one when it is in the top two. Never pick by comparing decode log-probs across languages: translations and hallucinations score higher. Evidence: `crates/echoctl/src/transcribe.rs::choose_language`
- R8. Keep a vocabulary-hinted decode only through `choose_hinted` against a plain decode of the same utterance (`hint_is_sane` length band, `hint_adds_terms` under half, `boost_is_heard`), as `Whisper::utterance` and `Client::openai_utterance` do from `GUARD_MIN_S`; Nemotron's boosted decode goes through `boost_is_heard`. A new hinted path decodes plain too and uses these checks. Evidence: `crates/echoctl/src/transcribe.rs::choose_hinted`
- R9. The hinted-vs-plain decode check (`hint_is_sane`, `hint_adds_terms`) applies to every utterance of at least `GUARD_MIN_S` (1 s) in both the live and the final pass; there is no separate live threshold. Evidence: `crates/echoctl/src/transcribe.rs::GUARD_MIN_S`
- R10. Build the hint with `build_prompt` as a bare, deduplicated comma list (people, meeting terms, `vocab::for_call`; at most 30 terms of 50 characters, 400 in all), never a sentence: a sentence prompt biases Ukrainian or Russian speech toward English translation, and a long one makes Whisper hear its names in silence. Evidence: `crates/echoctl/src/transcribe.rs::build_prompt`
- R11. In an `Engine` variant, drop each decoded sentence that `is_hallucination` or `is_word_list` flags before it becomes a `Piece`, as `Whisper::decode` (with `is_silence` on top) and `Nemo::utterance` do. The OpenAI-compatible path (`remote::pieces`) applies only the no-speech rule. Evidence: `crates/echoctl/src/transcribe.rs::Nemo.utterance`
- R12. Extend the transcription server protocol (`/v1/hello`, `/v1/utterance`) only with optional fields, as `Answer.words` is `#[serde(default)]`: older servers leave a new field out and older clients ignore it. Evidence: `crates/echoctl/src/remote.rs::Answer`
- R13. Return `remote::Refused` only for a final answer (`Client::check`: 401/403 a wrong key; 400/404/405/413 a bad request): `Transcriber::utterance` stops on it, while any other server error makes this computer decode for `SERVER_RETRY` and, with no local model, wait for the server up to `SERVER_WAIT`. Evidence: `crates/echoctl/src/transcribe.rs::Transcriber.utterance`
- R14. Name remote voices in `speakers::label`'s order: one other person in the call (`speaking::sole_remote`) names the whole remote track, else a name from `speakers.json` wins over the name Teams lit (`vote`) and over "Speaker N"; a 1:1 call named from its invite (`sole_other` in `write_transcript`) never reaches `label`. Evidence: `crates/echoctl/src/speakers.rs::label`
- R15. Put a Teams speaking-ring class confirmed in recorded trees in `KNOWN_MARKERS`; with two or more others `calibrate` takes a class only when it lights one tile at a time (`MIN_EXCLUSIVE`), so a layout or video-on class never wins on fit alone. Evidence: `crates/echoctl/src/speaking.rs::calibrate`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Write audio, `live.jsonl` or a transcript into a real call folder from a dev tool or test | Run `echoctl live-replay <wav> <dir>` into a new or empty folder (it marks it `.replay` and needs a model or server) | `replay_folder_ok` refuses any other folder; call folders hold someone's call | `crates/echoctl/src/live.rs::replay_folder_ok` |
| Transcribe a call that is still recording | Let the stop transcribe it (`watch::finish`), or run `echoctl transcribe` after | `transcribe` refuses the watcher's recording unless `FROM_WATCHER` names it | `crates/echoctl/src/transcribe.rs::transcribe` |

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| adding a speech engine | `crates/echoctl/src/transcribe.rs::Nemo.utterance` | restricted and sticky language, plain and boosted decode checked by `boost_is_heard`, the filters | `crates/echoctl/src/transcribe.rs::a_boosted_term_must_sound_like_what_was_heard` |

## Live notes

### Rules
- R1. Advance the live notes' `cursor` only when a summary is saved (`run_summary` writes `cursor: all.len()` with the notes); `livenotes::lines` reads only complete lines of `live.jsonl` and keeps a complete line that doesn't parse as an empty one, so the count never drifts. Evidence: `crates/echoctl/src/livenotes.rs::lines`
- R2. Turn a model's live-notes reply into a draft only through `parse_notes`: a reply with none of `now`, `decisions`, `actions`, `questions` is refused (saving it would wipe the draft), items are clipped to `MAX_ITEMS` × `MAX_ITEM_CHARS` and times kept within the call. Evidence: `crates/echoctl/src/livenotes.rs::parse_notes`
- R3. Run a short model job through `complete_claude_with`: `--tools ""`, `--strict-mcp-config`, `--no-session-persistence`, `--settings` the `lean_settings` file (`claudeMdExcludes: ["**"]`), `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1`, and `ai::wait_output_with`, which stops the child's group with `ai::stop_child` on the deadline or a cancel. Evidence: `crates/echoctl/src/livenotes.rs::complete_claude_with`
- R4. Re-check `summary_current(stamp, generation)` and `enabled()` after the model answers and before saving, as `run_summary` does: a summary started before a pause, a profile change or live notes being turned off is dropped. Evidence: `crates/echoctl/src/livenotes.rs::run_summary`

### Read first
| When you are | Exemplar | Why this one | Tests |
|---|---|---|---|
| testing live notes without a call | `crates/echoctl/src/livenotes.rs::a_summary_runs_end_to_end_and_failures_keep_the_draft` | a made-up `live.jsonl` in `testenv::folders`, a local profile against a test server, failures keeping the draft | exemplar is a test |
