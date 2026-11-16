# Capture and Transcription — reference

How recording, capture, transcription and live notes are built: the state, files, constants and functions the rules rely
on. It changes with the code, in the same PR. The rules are in `.ai-run/guides/capture/capture-and-transcription.md`.

## Arms

- Arms are written, cancelled, expired and taken under `watch::arm_lock`; `watch::take_arm(id)` returns true to the one
  caller that may start the recording.
- An arm is taken before `Recording::start` and used up even when the start fails (`watch::run_loop`).
- An arm expires after `ARM_EXPIRES` (15 minutes, `watch.rs`); an arm waiting for its meeting's call (Meeting matching)
  does not expire before the meeting ends.
- A scheduled meeting leaves the schedule once its recording starts; stopping never re-arms it.

## Opt-in

- A recording starts only after `watch::take_arm`; arms come from the user's choices: Record or `echoctl arm`
  (`watch::arm`), a calendar meeting's own choice, or a series rule the user set (`series.rs`: record always, ask or
  never; a meeting set by hand wins).
- Calendar meetings follow config: `ask` (offer "record?" about a minute before, the default) or `manual`.
- A call nobody armed: `watch::run_loop` emits `call_started` and, when `Config::ask_unscheduled` (config
  `unscheduled`: `ask`, the default, or `manual`), offers it (`app::offer_call`).

## Stop during a call

A user Stop during a call ends only the recording: the call's pending arm is held (`held_arm`) and no scheduled meeting
is armed (`stopped_in_call`) until the call ends. A memo never takes this path.

## Memos

- `recorder::Recording::start_memo`: refused while anything records; records only the default mic.
- A memo ends only on `stop.request`; a call during it is not recorded (`call_during_memo`).
- A memo's threads change through `memo::set_threads`, under the threads' lock.

## Meeting matching

- `watch::meetings_call` decides whether a call is an armed meeting's call.
- The window naming the meeting, or a call begun from `MATCH_EARLY_MIN` (10 minutes, `calendar.rs`) before the meeting,
  is its call.
- A call begun before that whose window doesn't name the meeting (another name, a channel's, none) makes the arm wait
  (`arm_waits_other_call`).
- A call window that can't be read at all starts the arm on the call going on; Record on a call already going on arms no
  meeting.

## Finishing a recording

`recorder::Recording::finish` halts every track before finishing any, finishes each even when another failed, and writes
the end under `calls::lock_meta`. The live worker is closed in `Recording::finish` and joined in the thread
`watch::finish_transcript` spawns.

## Call folders and the trash

- A new call gets a new folder (`recorder::call_dir`).
- Pruned frames move to `.echo/trash/<call>.frames/` with their index lines (`calls::prune_frames`); the app clears trash
  entries older than `TRASH_DAYS` (30, `calls::purge_trash`).

## Tracks

- Every track of a recording shares its one `t0`.
- `audio::write_track` keeps `mic.wav` and `remote.wav` sample-aligned across reopens, gaps padded with zeros.

## Capture sources

- A source sends mono `Chunk`s with its `epoch` and native `rate`.
- Its health returns `Health::Reopen` for a planned change and `Health::Restart` for a failure (`audio::Track::poll`);
  planned reopens skip the backoff.
- Exemplar: `audio::mic::MicStream::health`; test `audio::planned_reopens_skip_the_backoff_failures_dont`.

## Audio retention

Audio (`mic.wav`, `remote.wav`) is deleted in `transcribe::write_transcript`, after the transcript is written and unless
audio is kept (`keep_audio`), and noted through `calls::update_meta`.

## Transcription marker and failures

- `transcribe::take_marker` holds the in-progress marker for the whole time a call's transcript is written.
- A failure writes `<call>/transcribe.error`; recovery (`watch::recover`) skips such calls and calls with a transcript.
- A call still recording is transcribed by its stop (`watch::finish`), or with `echoctl transcribe` after.

## Decoding

- `transcribe::Transcriber` and `transcribe::Engine` hold decoding; the live worker (`live::run`) uses
  `final_transcriber`.
- Adding a speech engine: a `transcribe::Engine` variant (with its `EngineState`), a `models::Engine` variant, `CATALOG`
  entries and its arm in `models::runnable`. Exemplar: `transcribe::Nemo::utterance`; test
  `transcribe::a_boosted_term_must_sound_like_what_was_heard`.
- A runtime-only speech library loads with `libloading` at first use, as `nemo.rs` does (`nemo::load_api`);
  `available()` is false when it is missing.
- `is_hallucination` and `is_word_list` flag sentences an `Engine` drops before they become a `Piece`.

## Vocabulary hint

- `transcribe::build_prompt` builds the hint: a bare list of terms.
- `transcribe::choose_hinted` keeps a hinted decode only against a plain decode of the same utterance.
- `transcribe::GUARD_MIN_S` (1.0 s) is the hinted-decode guard, used by the live and the final pass.

## Language and model

- `call_language::for_new_call` sets a call's language when its recording starts; `call_language::change` changes it by
  hand.
- `models::for_call` gives the model for a call's language; `Transcriber::set_language` swaps it.

## Transcription server

- The server the user connected: `remote::Client::configured` (`remote::connect`, `remote::connect_openai` for an
  OpenAI-compatible one; `remote::disconnect`).
- Protocol: `/v1/hello`, `/v1/utterance` (`remote::Answer`).
- `remote::Refused` is the final answer (a wrong key, a bad request); `transcribe::Transcriber::utterance` falls back to
  this computer's model on any other error.

## Learned vocabulary in config

Every saved config writes `learned` (`[]` when empty) (`config::learned_always`), so a config this build saved is never
taken for one from before learned terms; `vocab::migrate` migrates only a config without the key.

## Speaking ring

- `speaking::calibrate` picks Teams' speaking-ring class.
- `KNOWN_MARKERS` lists classes confirmed in recorded trees; `MIN_EXCLUSIVE` (0.6) is how often a class must light one
  tile at a time when there are two or more others.

## Replaying audio

`echoctl live-replay <wav> <dir>` writes into a new or empty folder, marks it `.replay` (`live::replay_folder_ok`) and
needs a model or a transcription server.

## Live notes

- `livenotes::lines` reads only complete lines of `live.jsonl`; the `cursor` moves when a summary is saved.
- `livenotes::parse_notes` turns a model's reply into a draft.
- `livenotes::run_summary` re-checks `summary_current` and `enabled()` after the model answers.
- A short model job runs through `claude_session::complete_claude_with`: no tools, no MCP, no session persistence, no
  memory, with a deadline that stops the child's group.
- On a Claude profile: one Claude Code session per recording (`livenotes::agent::ensure`), allowed only the call-bound
  MCP server's (`echoctl live-mcp --call <dir>`) two tools, `wait_for_new_lines` and `update_live_notes`.
- Testing without a call: `livenotes::a_summary_runs_end_to_end_and_failures_keep_the_draft`.

## Exemplars

| When you are | Exemplar | Tests |
|---|---|---|
| changing arm, record or stop | `watch::run_loop` | `watch::a_recording_takes_only_its_own_arm` |
| adding or changing a capture source | `audio::mic::MicStream::health` | `audio::planned_reopens_skip_the_backoff_failures_dont` |
| adding a speech engine | `transcribe::Nemo::utterance` | `transcribe::a_boosted_term_must_sound_like_what_was_heard` |
| testing live notes without a call | `livenotes::a_summary_runs_end_to_end_and_failures_keep_the_draft` | exemplar is a test |
