# Claude Code Runs — reference

How Echo starts Claude Code on AI profiles. It changes with the code, in the same PR. The rules are in
`.ai-run/guides/integrations/claude-code-runs.md`.

## Starting Claude Code

- `ai::claude_command` (or `ai::prepare_claude`) builds a Claude Code command for a profile; a bare `app::claude_bin()`
  is for `--version` and plugin-management commands.
- `local` / `claude` jobs run with `--setting-sources user` (never on `claude auth …`); `ai::policy_conflict` refuses them
  when admin policy could route.
- Children start in their own session (`ai::detach`); only `echoctl ai login`'s child stays in the terminal's foreground.

## Profiles

- `ai::for_call` / `ai::call_profile_id` give a call's profile; an unreadable `meta.json` or a missing profile is an
  error.
- `ai::pin` writes the call's profile into `meta.json` at recording start.
- `ai::lease` is kept by the `Ready` that `ai::ready` returns.
- During a sign-in `ai::for_call` fails at once with `SigningIn`; live jobs are deferred (`livenotes::defer`).
- `ai::accept_account`: a `claude` profile signed in to another account than the one recorded waits for the user.

## Routing variables

A routing environment-variable family is listed in two places: `ai::routing_var` (the process environment) and
`ai::UNSET_ROUTING` (the login-shell `sed` filter).

## Minutes runs

- `notes::admit` admits a minutes run; `notes::auto_due` re-checks an automatic run right before Claude Code starts.
- A run writes `actions.new-<run>.json`; `notes::finish_actions` applies only that file (never the generic
  `actions.new.json`) through `calls::apply_actions`, which keeps each item's current status.

## Tests

`ai::fake::install(root)` installs a scripted fake Claude Code.
