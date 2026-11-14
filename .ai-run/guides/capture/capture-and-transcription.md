# Capture and Transcription

**Project**: Echo · **Scope**: recording state machine, capture tracks, transcription engines and guards, live notes

**Reference**: `.ai-run/reference/capture/capture-and-transcription.md` — the state, files, constants and functions
these rules rely on (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Recording

### Rules
- R1. An arm is written, cancelled, expired or taken only under the arm lock, and a recording starts only after it took its arm. Evidence: `watch::take_arm` (ref: Arms)
- R2. An arm is single-use: taken before the recording starts, used up even when the start fails, and it expires (an arm waiting for its meeting's call, R10, not before the meeting ends); a scheduled meeting leaves the schedule once its recording starts, and stopping never re-arms it. Evidence: `watch::run_loop` (ref: Arms)
- R3. A user Stop during a call ends only the recording: nothing arms again until the call ends; a memo never takes this path. Evidence: `watch::run_loop` (ref: Stop during a call)
- R4. One recording at a time: a memo is refused while anything records, records only the default mic, ends only on the user's stop, and a call during it is not recorded. Evidence: `recorder::Recording::start_memo` (ref: Memos)
- R5. A recording ends only through `Recording::finish`, every track halted before any is finished and each finished even when another failed; what was saved is transcribed even when it returns an error. Evidence: `recorder::Recording::finish` (ref: Finishing a recording)
- R6. Never reuse a call folder. Evidence: `recorder::call_dir`
- R7. Pruned frames move to the trash, never deleted. Evidence: `calls::prune_frames` (ref: Call folders and the trash)
- R8. A memo's threads change only through `memo::set_threads`; the user's pins are never taken back. Evidence: `memo::set_threads`
- R9. An arm for a meeting starts only on that meeting's call when the call window can be read: the window naming the meeting, or a call begun shortly before it, is its call. Evidence: `watch::meetings_call` (ref: Meeting matching)
- R10. A call begun before that whose window doesn't name the meeting makes the arm wait until the user is on its call; a name alone never does. Evidence: `watch::meetings_call` (ref: Meeting matching)
- R11. A call window it can't read at all starts the arm on the call going on, and Record on a call already going on arms no meeting. Evidence: `watch::meetings_call`
- R12. Recording is opt-in per meeting: a call is recorded only when the user chose it (Record, its meeting, or a series the user set to always); a call nobody chose is at most offered. Evidence: `watch::take_arm`, `app::offer_call` (ref: Opt-in)

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Join the live worker or decode on the watcher's thread | Close it when the recording finishes and join it on the finishing thread (ref: Finishing a recording) | `watch::finish_transcript` |

## Capture tracks

### Rules
- R1. Every track of a recording shares one start time and stays sample-aligned across reopens. Evidence: `audio::write_track` (ref: Tracks)
- R2. A capture source sends mono chunks with its epoch and native rate, and tells a planned change apart from a failure. Evidence: `audio::Track::poll` (ref: Capture sources)
- R3. Capture, accessibility and permissions work on macOS and Windows both: a change to them lands in both OSes' files. Evidence: `permissions::report` (ref: `.ai-run/reference/architecture/architecture.md`, Per-OS files)

## Transcription

### Rules
- R1. Audio is deleted only after the transcript is written, unless the user keeps it, and the deletion is noted in the call's metadata. Evidence: `transcribe::write_transcript` (ref: Audio retention)
- R2. Writing a call's transcript holds the transcription marker for the whole time. Evidence: `transcribe::take_marker`
- R3. A transcription failure is recorded in the call and never retried by itself; recovery skips such calls and calls with a transcript. Evidence: `watch::recover` (ref: Transcription marker and failures)
- R4. Decoding changes go in the transcriber, never in the live worker, which decodes through the final transcriber. Evidence: `live::run` (ref: Decoding)
- R5. A speech engine is one `transcribe::Engine` variant, registered with the model catalog; the transcription server decodes through the same `Engine`. Evidence: `transcribe::Engine` (ref: Decoding)
- R6. A runtime-only speech library loads at first use and reports itself unavailable when missing; never a link-time dependency. Evidence: `nemo::load_api` (ref: Decoding)
- R7. A vocabulary-hinted decode is kept only through `choose_hinted` against a plain decode of the same utterance; a new hinted path decodes plain too and uses the same checks. Evidence: `transcribe::choose_hinted`
- R8. An engine drops sentences flagged as hallucinations or word lists before they become transcript pieces. Evidence: `transcribe::Nemo::utterance` (ref: Decoding)
- R9. The transcription server protocol grows only by optional fields. Evidence: `remote::Answer` (ref: Transcription server)
- R10. Only a final answer from the server (a wrong key, a bad request) is a refusal; any other server error falls back to this computer's model. Evidence: `transcribe::Transcriber::utterance` (ref: Transcription server)
- R11. A call's language is set when its recording starts and changed only by hand; nothing detects it. Evidence: `call_language::change` (ref: Language and model)
- R12. The vocabulary hint is a bare list of terms, never a sentence. Evidence: `transcribe::build_prompt`
- R13. The hinted-decode guard applies to the live and the final pass alike. Evidence: `transcribe::GUARD_MIN_S` (ref: Vocabulary hint)
- R14. A Teams speaking-ring class is taken when confirmed in recorded trees, or, with two or more others, only when it lights one tile at a time; never on fit alone. Evidence: `speaking::calibrate` (ref: Speaking ring)
- R15. A call has one language, and its model is the one given for that language; a change by hand swaps the model, never per utterance. Evidence: `models::for_call` (ref: Language and model)
- R16. Every saved config writes the learned-vocabulary key, even empty; only a config without it is migrated. Evidence: `config::learned_always` (ref: Learned vocabulary in config)
- R17. Transcription runs on this computer or on a transcription server the user connected; call audio goes nowhere else. Evidence: `transcribe::Transcriber::utterance`, `remote::Client::configured` (ref: Transcription server)

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Write audio, a live transcript or a transcript into a real call folder from a dev tool or test | Replay into a new or empty folder (ref: Replaying audio) | `live::replay_folder_ok` |
| Transcribe a call that is still recording | Let the stop transcribe it, or transcribe it after (ref: Transcription marker and failures) | `transcribe::transcribe` |

## Live notes

### Rules
- R1. The live notes' cursor advances only when a summary is saved, and only complete lines of the live transcript are read. Evidence: `livenotes::lines`
- R2. A model's live-notes reply becomes a draft only through `parse_notes`. Evidence: `livenotes::parse_notes`
- R3. A short model job runs with no tools, no MCP, no session persistence and no memory, under a deadline that stops the child's group. Evidence: `claude_session::complete_claude_with` (ref: Live notes)
- R4. A summary is re-checked as current after the model answers and before it is saved. Evidence: `livenotes::run_summary`
- R5. Live notes on a Claude profile run in one Claude Code session per recording, allowed only the tools of the call-bound server; the session's text is never read, and it is stopped once the recording ends. Evidence: `livenotes::agent::ensure` (ref: Live notes)
