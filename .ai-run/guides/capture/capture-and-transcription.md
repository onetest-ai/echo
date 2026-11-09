# Capture and Transcription

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: recording state machine, capture tracks, transcription engines and guards, live notes

## Recording

### Rules
- R1. Write, cancel, expire or take an arm only under `watch::arm_lock`, and start a recording only after `take_arm(id)` returned true. Evidence: `watch::take_arm`
- R2. An arm is single-use: taken before `Recording::start`, used up even when the start fails, expired after `ARM_EXPIRES`; a scheduled meeting leaves the schedule once its recording starts, and stopping never re-arms it. Evidence: `watch::run_loop`
- R3. A user Stop during a call ends only the recording: the call's pending arm is held (`held_arm`) and no scheduled meeting is armed (`stopped_in_call`) until the call ends; a memo never takes this path. Evidence: `watch::run_loop`
- R4. One recording at a time: a memo is refused while anything records, records only the default mic, ends only on `stop.request`, and a call during it is not recorded (`call_during_memo`). Evidence: `recorder::Recording::start_memo`
- R5. A recording ends only through `Recording::finish` (every track halted before any is finished, each finished even when another failed, the end written under `calls::lock_meta`); what was saved is transcribed even when it returns an error. Evidence: `recorder::Recording::finish`
- R6. Never reuse a call folder. Evidence: `recorder::call_dir`
- R7. Pruned frames move to `.echo/trash/<call>.frames/` with their index lines, never deleted. Evidence: `calls::prune_frames`
- R8. A memo's threads change only through `memo::set_threads`, under the threads' lock; the user's pins are never taken back. Evidence: `memo::set_threads`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Join the live worker or decode on the watcher's thread | Close it in `Recording::finish` and join it in the thread `watch::finish_transcript` spawns | `watch::finish_transcript` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| changing arm, record or stop | `watch::run_loop` | `watch::a_recording_takes_only_its_own_arm` |

## Capture tracks

### Rules
- R1. Every track of a recording shares its one `t0`; `write_track` keeps `mic.wav` and `remote.wav` sample-aligned across reopens (gaps padded with zeros). Evidence: `audio::write_track`
- R2. A capture source sends mono `Chunk`s with its `epoch` and native `rate`; its health returns `Health::Reopen` for a planned change and `Health::Restart` for a failure. Evidence: `audio::Track::poll`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| adding or changing a capture source | `audio::mic::MicStream::health` | `audio::planned_reopens_skip_the_backoff_failures_dont` |

## Transcription

### Rules
- R1. Audio (`mic.wav`, `remote.wav`) is deleted only in `transcribe::write_transcript`, after the transcript is written and unless audio is kept (`keep_audio`), and noted through `calls::update_meta`. Evidence: `transcribe::write_transcript`
- R2. Writing a call's transcript holds `transcribe::take_marker` for the whole time. Evidence: `transcribe::take_marker`
- R3. A transcription failure writes `<call>/transcribe.error` and is never retried by itself; recovery (`watch::recover`) skips such calls and calls with a transcript. Evidence: `watch::recover`
- R4. Decoding changes go in `transcribe::Transcriber` / `Engine`, never in `live.rs`: the live worker uses `final_transcriber`. Evidence: `live::run`
- R5. A speech engine is a `transcribe::Engine` variant (with its `EngineState`) plus a `models::Engine` variant, `CATALOG` entries and its arm in `models::runnable`; the transcription server decodes through the same `Engine`. Evidence: `transcribe::Engine`
- R6. A runtime-only speech library loads with `libloading` at first use (as `nemo.rs`), `available()` false when missing; never a link-time dependency. Evidence: `nemo::load_api`
- R7. A vocabulary-hinted decode is kept only through `choose_hinted` against a plain decode of the same utterance; a new hinted path decodes plain too and uses the same checks. Evidence: `transcribe::choose_hinted`
- R8. An `Engine` variant drops sentences that `is_hallucination` or `is_word_list` flags before they become a `Piece`. Evidence: `transcribe::Nemo::utterance`
- R9. The transcription server protocol (`/v1/hello`, `/v1/utterance`) grows only by optional fields. Evidence: `remote::Answer`
- R10. `remote::Refused` is returned only for a final answer (a wrong key, a bad request); any other server error falls back to this computer's model. Evidence: `transcribe::Transcriber::utterance`
- R11. Never pick a call's language by comparing decode log-probabilities across languages. Evidence: `call_language::CallLanguage`
- R12. The vocabulary hint is a bare list of terms from `build_prompt`, never a sentence. Evidence: `transcribe::build_prompt`
- R13. The hinted-decode guard (`GUARD_MIN_S`) applies to the live and the final pass alike. Evidence: `transcribe::GUARD_MIN_S`
- R14. A Teams speaking-ring class confirmed in recorded trees goes in `KNOWN_MARKERS`; with two or more others, a class is taken only when it lights one tile at a time (`MIN_EXCLUSIVE`), never on fit alone. Evidence: `speaking::calibrate`
- R15. A call has one language: an `Engine` runs no language detection when one language is allowed, and a call's language is decided once (`CallLanguage`), never per utterance. Evidence: `transcribe::detect_language`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Write audio, `live.jsonl` or a transcript into a real call folder from a dev tool or test | Run `echoctl live-replay <wav> <dir>` into a new or empty folder (it marks it `.replay` and needs a model or server) | `live::replay_folder_ok` |
| Transcribe a call that is still recording | Let the stop transcribe it (`watch::finish`), or run `echoctl transcribe` after | `transcribe::transcribe` |

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| adding a speech engine | `transcribe::Nemo::utterance` | `transcribe::a_boosted_term_must_sound_like_what_was_heard` |

## Live notes

### Rules
- R1. The live notes' `cursor` advances only when a summary is saved; `livenotes::lines` reads only complete lines of `live.jsonl`. Evidence: `livenotes::lines`
- R2. A model's live-notes reply becomes a draft only through `parse_notes`. Evidence: `livenotes::parse_notes`
- R3. A short model job runs through `complete_claude_with` (no tools, no MCP, no session persistence, no memory) with a deadline that stops the child's group. Evidence: `claude_session::complete_claude_with`
- R5. Live notes on a Claude profile run in one Claude Code session per recording, allowed only the two tools of the call-bound server (`wait_for_new_lines`, `update_live_notes`); the session's text is never read, and it is stopped once the recording ends. Evidence: `livenotes::agent::ensure`
- R4. A summary is re-checked as current (`summary_current`, `enabled()`) after the model answers and before it is saved. Evidence: `livenotes::run_summary`

### Read first
| When you are | Exemplar | Tests |
|---|---|---|
| testing live notes without a call | `livenotes::a_summary_runs_end_to_end_and_failures_keep_the_draft` | exemplar is a test |
