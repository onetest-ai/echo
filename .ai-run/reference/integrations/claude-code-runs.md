# Claude Code Runs — reference

How Echo starts Claude Code on AI profiles. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/integrations/claude-code-runs.md`.

## Starting Claude Code

- `ai::claude_command` (or `ai::prepare_claude`) builds a Claude Code command for a profile; a bare `app::claude_bin()`
  is for `--version` and plugin-management commands.
- `local` / `claude` jobs run with `--setting-sources user` (never on `claude auth …`); `ai::policy_conflict` refuses them
  when admin policy could route.
- Children start in their own session (`ai::detach`); only `echoctl ai login`'s child stays in the terminal's foreground.
- A thread's sync gives its runs their text only on stdin, one stream-json message (`--input-format stream-json`), and
  their fixed instructions in a file (`--append-system-prompt-file`, removed after the run); no skill
  (`--disable-slash-commands`) (`projectsync::args`).

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
  `ANTHROPIC_SMALL_FAST_MODEL` (each name its own model when set, else the profile's minutes model). Set by the user:
  `echoctl ai set <id> --model <name>=<model>` (empty clears), Settings → AI & minutes → Models… (`profile_models`),
  or seeded from a plugin's wrapper offer when the user adds it (`uses.ai.wrapper.models`). Not on `system` (the user's
  own Claude Code setup).

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
  `ai::wrapper_ready` with `ai::wrapper::Expected` (a Claude Code `guard::new_enough`, the permission mode Echo set — `bypassPermissions` with
  `--dangerously-skip-permissions`, else `--permission-mode`; none set: anything but `bypassPermissions`; not checked
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

## Tests

`ai::fake::install(root)` installs a scripted fake Claude Code.
