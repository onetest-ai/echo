# Claude Code Runs — reference

How Echo starts Claude Code on AI profiles. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/integrations/claude-code-runs.md`.

## Starting Claude Code

- `ai::claude_command` (or `ai::prepare_claude`) builds a Claude Code command for a profile; a bare `app::claude_bin()`
  is for `--version` and plugin-management commands.
- `local` / `claude` jobs run with `--setting-sources user` (never on `claude auth …`); `ai::policy_conflict` refuses them
  when admin policy could route. A plugin's run in a folder it was given names its own, `--setting-sources user,project`
  as its last arguments (`claudejob::in_folder`; `ai::build_command` then adds none), and is refused when that folder's `.claude/settings.json`
  or `.claude/settings.local.json` could route (`ai::routes`) or isn't JSON. A plugin's session in Echo's terminal (`terminal::start_plugin`) is built
  the same way, keeping its conversation (`claudejob::in_folder_session`: no `--no-session-persistence`).
- Children start in their own session (`ai::detach`); only `echoctl ai login`'s child stays in the terminal's foreground.
- A thread's sync gives its runs their text only on stdin, one stream-json message (`--input-format stream-json`), and
  their fixed instructions in a file (`--append-system-prompt-file`, removed after the run); no skill
  (`--disable-slash-commands`) (`projectsync::args`).
- A plugin's run (`claudejob::run_with`) with pictures sends them and its prompt as one stream-json message on stdin
  (`-p --input-format stream-json`), else its prompt after `-p`; streamed (`onText`) it adds
  `--include-partial-messages`; in a conversation (`resume`) it drops `--no-session-persistence` and continues the
  session it keeps with `--resume <session>`. Every event of a run reaches `projectsync::finish_run_watched`'s callback.
  Measured on Claude Code 2.1.328: base64 image blocks in a stream-json message are read in `-p`; text arrives as
  `stream_event` `content_block_delta` / `text_delta` events (`parent_tool_use_id` null for Claude's own); `--resume
  <id>` in `-p` answers in the same session id; a `--resume` of a session it doesn't have prints "No conversation found
  with session ID: <id>" on stderr and a result with `is_error` and that text in `errors`, then exits 1.
- A plugin's conversation keeps a transcript: Claude Code writes it to the profile's Claude Code folder
  (`<CLAUDE_CONFIG_DIR>/projects/<folder>/<session>.jsonl` and `<session>/`; a `local` or `claude` profile's own folder,
  else the user's `~/.claude` or `CLAUDE_CONFIG_DIR`), listed by the folder the run started in. A conversation with no
  tools starts in `<state>/plugins/.runs/<plugin>/talk`, never the data folder, so `/resume` in the Claude · Echo folder
  session doesn't list it; one with tools starts in the data folder and one in a given folder there, and those lists
  show it. `claudejob::forget_record` removes the record and that transcript (`ctx.claude.forget`, the plugin removed
  with its data, a session Claude Code no longer has). Pictures are capped by the smallest limits of the platforms a
  profile may reach (Amazon Bedrock: 5 MB base64-encoded a picture, 20 MB a request; the Claude API: 10 MB and 128 MB;
  more than 20 pictures in a request: smaller pictures only), counted across a conversation's turns, since Claude Code
  sends its earlier pictures again (`claudejob::IMAGES_TOTAL`, `IMAGES_TOTAL_BYTES`).
- The folder session, a plugin's session (on the folder session's setting), a routine, Yolo and a plugin's folder run (on the
  plugin's `tools`, else the routines' setting) get their permission flags
  from one helper, `folder_args::permission_args(setting, profile, model, may_edit)` → (flags, words for the run's
  record): Ask each time none; Skip all permission checks `--dangerously-skip-permissions`; Auto
  `--permission-mode auto` where `folder_args::auto_mode_works` (a `claude` or `system` profile, its model Claude's
  name or alias (`opusplan`, `sonnet[1m]`, `opus[1m]`), Claude Code's default, or an id of Opus / Sonnet 4.6+, Haiku 5.5+ or Fable — Bedrock / Vertex ids, with
  `anthropic.` or `@`, Sonnet 5+ or Opus 4.7+; an id it can't read: no), else `--permission-mode acceptEdits` (may
  edit) or `dontAsk` (read-only). Without auto mode Claude Code starts in its manual mode, where a headless run does
  nothing and a wrapper's start event (`default`) wouldn't match Echo's `auto`. A routine's or Yolo's run checks its
  start event's `permissionMode` against the mode it asked for (`claudejob::finish`, through
  `projectsync::finish_run_watched`): when they differ — auto mode not on for that model, provider or account — its log
  and its record (`permissions.json`, `mode`) say so ("Claude Code ran in manual mode, not auto: …"). `folder_args::auto_on_profiles` gives
  what Auto gets per profile (Settings' card, `echoctl config`). Measured on Claude Code 2.1.326: `-p` with `--permission-mode auto` runs headless, its `system/init` shows
  `permissionMode: auto`, and a Bash command outside `--allowedTools` ran without a prompt. The fake Claude Code
  (`ai::fake`, the test data's `bin/claude`) takes any flags.

## Profiles

- `ai::for_call` / `ai::call_profile_id` give a call's profile; an unreadable `meta.json` or a missing profile is an
  error.
- `ai::pin` writes the call's profile into `meta.json` at recording start.
- `ai::lease` is kept by the `Ready` that `ai::ready` returns.
- During a sign-in `ai::for_call` fails at once with `SigningIn`; live jobs are deferred (`livenotes::defer`).
- `ai::accept_account`: a `claude` profile signed in to another account than the one recorded waits for the user.
- A profile's `models` (`ai::Models`: `haiku`, `sonnet`, `opus`, each optional) is what it runs for Claude's three
  names: `Profile::model(name)` is the one set, else `name` as it is (any other model name passes unchanged). Applied
  where a model reaches the provider: `claude_session::complete_claude_with`, `livenotes::complete_local_body`, the
  live-notes agent's `--model`, minutes (`notes`), thread sync (`projectsync`), routine and Yolo runs (`claudejob`), the
  local terminal session's `--model`, and for a `local` profile its `ANTHROPIC_DEFAULT_*_MODEL` /
  `ANTHROPIC_SMALL_FAST_MODEL` (each name its own model when set, else the profile's minutes model). On a `claude` or
  `wrapper` profile only what Echo asks for is mapped: Claude Code's own background requests and the folder and call
  sessions' (no `--model` there) keep Claude's names. Set only by the user, all names checked then written at once
  (`ai::set_model_names`; a model is one word, not starting with "-", not itself one of the three): `echoctl ai set
  <id> --model <name>=<model>` (empty clears), Settings → AI & minutes → Models… (`profile_models`); never on `system`
  (the user's own Claude Code setup), never inside a run Echo started (nor `--notes-model`).

## Routing variables

A routing environment-variable family is listed in two places: `ai::routing_var` (the process environment) and
`ai::UNSET_ROUTING` (the login-shell `sed` filter).

## Minutes runs

- `notes::admit` admits a minutes run; `notes::auto_due` re-checks an automatic run right before Claude Code starts.
- A run writes `actions.new-<run>.json`; `notes::finish_actions` applies only that file (never the generic
  `actions.new.json`) through `calls::apply_actions`, which keeps each item's current status.

## Wrapper profiles

A `wrapper` profile (`ai::Kind::Wrapper`, `ai::wrapper`) starts a command the user installed in place of `claude`,
unmanaged like `system`: the user's own setup, the hook check (`ai::hooks_conflict`, `system`'s folders), never the
routing-policy check.

- Added by `echoctl ai add-wrapper <id> --command <program> [--name …]` (`ai::add_wrapper`), refused inside a run Echo
  started. `ai::wrapper::admit`: the command is found as the run's login shell finds it (`command -v`, the name as
  `$0`; with `ECHOCTL_NOTES_NO_SHELL`, on this process's own `PATH`, no shell asked: runs then start it directly), links followed; refused when it lies in the data folder or Echo's own folder (plugins' packages, My plugins
  and their stores are in them; compared ignoring case on macOS and Windows) or its `--version`, asked as a run would
  start it (`claude_command`: the login shell), names no Claude Code. `ai.json` keeps the absolute path
  (`wrapper.command`) and the plugin that offered it (`wrapper.offered_by`); a stored one without a full path fails
  only that profile's jobs.
- A plugin's offer (`uses.ai.wrapper`, plugin API reference): `ai::offered` lists the enabled plugins' offers with the
  profile taken from each and, when the plugin now names another program than that profile took
  (`wrapper.offered_command`), its `update`; `ai::add_offered_wrapper` adds one with the same checks, its id made from
  the plugin's; `ai::update_wrapper` takes the new program on the user's yes. Settings → AI & minutes: offers, Add a
  wrapper… (Check shows the path and version found, then Add profile), Use it on a profile its plugin's update changed
  (`wrapper_check`, `wrapper_add` — it adds only the path Check showed, `wrapper_update`, their outcome in the settings
  snapshot's `wrapper`). The profile keeps the plugin's id, marketplace and name (`wrapper.offered_by`,
  `offered_market`, `offered_name`); an offer whose program is a wrapper profile already isn't shown.
- Inside a run Echo started, a wrapper profile is never made the default (`ai default`), a call's (`set_call_profile`),
  a thread's (`project set --profile`) or the next recording's (`arm --ai`, an `arm --meeting` file's `ai_profile`,
  `memo start --ai`) (`ai::wrapper_in_run`); a plugin's job names one only when it is the user's
  default (`ai::plugin_may_name`).
- Every start uses that path (`ai::program_for`): a missing file fails the job. `claude plugin …` (a plugin's
  `claude.has_plugin` / `install_plugin`) runs the bare `claude`, so a wrapper keeping its own Claude Code folder
  answers about another one.
- A print run (`-p`) goes through `echoctl wrapper-run -- <wrapper> <args…>` (`ai::wrapper::relay`): the prompt comes
  in `ECHOCTL_WRAP_PROMPT` (or the relay's stdin) and goes to the wrapper only as a stream-json message on stdin
  (after `-p`, only a plain flag word is taken for a flag); `--system-prompt` / `--append-system-prompt` become
  `…-file` in Echo's run folder; `--output-format json` becomes stream-json (the result line is the same), any other
  but stream-json is refused. The wrapper leads its own process group: a mismatch, a timeout or the relay stopped
  (TERM: it stops that group — waiting for one still being started — and removes the run's files and mark) takes down what it started. The run's settings file gets the proof's hooks
  (`ai::wrapper::proof_settings`): a `SessionStart` hook `echoctl proof hello <value>` and a `PreToolUse` gate
  `echoctl proof gate <value>` (`--deny-all` when Echo set no tools and no MCP server). `<value>`: 32 random hex digits
  per run. The prompt is sent once the hook's answer with that value is seen; the run's `system/init` is checked by
  `ai::wrapper_ready` with `ai::wrapper::Expected` (the permission mode Echo set — `bypassPermissions` with
  `--dangerously-skip-permissions`, else `--permission-mode` (`auto` for a session on Auto); none set: anything but `bypassPermissions`; not checked
  when the run has no tool and no MCP server — no built-in tool beyond Echo's
  `--tools`, and with `--strict-mcp-config` no MCP server or tool beyond Echo's `--mcp-config`), then marked
  (`.plugins/.proof/<value>`, removed at the run's end, never by age). The run's files are named by another random id
  (`minutes-run/proof-<id>…`), and a day-old one is swept. The gate (`guard::checked`) holds a tool call up to 8 s for the
  mark, then refuses it. A mismatch, or no answer within 90 s at either step, stops the run: its stderr and a last
  result line `Echo stopped this run: …` name the wrapper.
- Measured on Claude Code 2.1.325: with stream-json input held back, `SessionStart` hooks run and report
  (`hook_response`) before any input; `system/init` comes only with the first message.
- A terminal (`ai::wrapper::terminal_proof`): the folder session and the call session get the proof's hooks, the hello
  with `--mark` (its answer is the check; its mark `<value>.t`); the mark goes when the session ends
  (`terminal::unmark_on_exit`) or Echo stops it (`terminal::close`), and every `.t` mark when Echo starts
  (`ai::wrapper::clear_terminal_marks`). The call
  session's first prompt is Echo's own words naming the call's folder, no title (`terminal::wrapper_first_prompt`),
  typed in once the mark is there as one bracketed paste with no control characters (`terminal::type_in_when_marked`,
  within 90 s), never on the command line. The call session's version check asks the
  wrapper (`ai::wrapper::claude_version`).

## Telemetry and traces

- A print run (`-p`) is a Claude Code run of a trace (`telemetry::claude_run`, called by `ai::prepare_claude`): a span
  `claude.run`, a child of its thread's current span (a job's, `job.<id>`), else a trace of its own; its attributes
  the run's arguments, the text of the `--system-prompt-file` / `--append-system-prompt-file` it names, the profile,
  the folder and `echo.telemetry` (`echo`, `configured elsewhere` or `off`). Its stdin message (or, through a wrapper,
  its prompt) is an event `claude.input` (`telemetry::claude_input`); each stream-json event, whole, an event
  `claude.<type>[.<subtype>]` (`run_log::write`, `claude_session`, the live notes agent through
  `telemetry::claude_event_of`); its result ends the span with its tokens, cost, turns and model. `--version`,
  sign-ins and terminal sessions get none.
- Claude Code's own telemetry is pointed at this process (`telemetry::receiver`: OTLP over HTTP, JSON only, on
  `127.0.0.1` and a port the system picks): `CLAUDE_CODE_ENABLE_TELEMETRY=1`, `CLAUDE_CODE_ENHANCED_TELEMETRY_BETA=1`
  (its traces), `OTEL_TRACES_EXPORTER` / `OTEL_LOGS_EXPORTER` `otlp`, `OTEL_EXPORTER_OTLP_PROTOCOL` `http/json`,
  `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_LOGS_EXPORT_INTERVAL` 2000, `OTEL_RESOURCE_ATTRIBUTES`
  (`echo.trace`, `echo.span`, `echo.job`) and `TRACEPARENT` (the run's span: Claude Code's `claude_code.interaction`
  is its child). No `OTEL_LOG_*`: its prompt and reply logging stays off. None of them when something else configures
  its telemetry (`telemetry::configured_elsewhere`: a `telemetry::telemetry_var` — `OTEL_*`,
  `CLAUDE_CODE_ENABLE_TELEMETRY`, `CLAUDE_CODE_ENHANCED_TELEMETRY_BETA` — in this process's environment or in the
  `env` of the profile's own settings (a `local` / `claude` profile's folder, else `CLAUDE_CONFIG_DIR` or `~/.claude`),
  the folder's `.claude/settings.json` / `settings.local.json`, or the organisation's policy (`ai::policy_docs`: the
  managed settings and `managed-settings.d/`, macOS managed preferences `com.anthropic.claudecode`, Windows policy keys,
  the server-managed `remote-settings.json` in the profile's folder, its `env` or `settings.env`) — a policy source that
  is there but can't be read counts too), nor on a wrapper profile (`echo.telemetry` "the wrapper's own": it may set its own where
  Echo can't see); through the login shell they pass as `ECHOCTL_T_<name>` and are exported only when the start-up
  files set none, the settings folder they name (`CLAUDE_CONFIG_DIR`: its `settings.json` and `remote-settings.json`)
  mentions none, and `grep` is there to tell (`ai::shell_script`).
- The receiver takes only `application/json` sent to its own address (`Host` 127.0.0.1 or localhost with its port),
  and files only traces of runs this process started in the last two days (`telemetry::known`); at most 8
  connections at once, a minute each, 10 s between reads, 16 KB a line, 16 MB a body; a span keeps only its
  resource's `service.*` and `echo.*`. A run left without a result ends with its parent span (`ok` false, "Claude
  Code ended without a result"), or after two days; the live notes session is detached from its job
  (`telemetry::detach_last_run`: its own trace's root, ended by its stream's result) and its job line written by
  `jobs::record_run`. A streamed run's partial-text events (`stream_event`) aren't kept.
- Measured on Claude Code 2.1.329 (#859): a settings file's `env` overrides the process environment (Echo's endpoint
  and a `--settings` file naming another: everything went to the other, with its resource attributes); log records
  carry `traceId` and `spanId`; `TRACEPARENT` makes its root span a child; through the CodeMie wrapper the same.

## Tests

`ai::fake::install(root)` installs a scripted fake Claude Code.
