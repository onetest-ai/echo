# Claude Code Runs

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: starting Claude Code on AI profiles, strict routing, leases, the fake in tests

## Starting Claude Code

### Rules
- R1. Start Claude Code on call text, for a sign-in check or a sign-in through `ai::claude_command` (or `ai::prepare_claude`), which applies the profile's routing and policy checks; a bare `app::claude_bin()` is used only for `--version` and plugin-management commands (`terminal::claude_new_enough`, `app::update_plugins`, `claude_setup`). Evidence: `ai::claude_command`
- R2. Pick a call's AI profile with `ai::for_call` / `ai::call_profile_id`: an unreadable `meta.json` or a missing profile is an error, never a fallback to another profile. Evidence: `ai::call_profile_id`
- R3. When a routing environment-variable family changes, change both `ai::routing_var` (the process environment) and `ai::UNSET_ROUTING` (the login-shell `sed` filter). Evidence: `ai::UNSET_ROUTING`
- R4. Hold the profile's lease (`ai::lease`, kept by the `Ready` that `ai::ready` returns) for the whole job. Evidence: `ai::ready`
- R5. Pin a call's AI profile into `meta.json` at recording start. Evidence: `ai::pin`
- R6. Run `local` / `claude` jobs with `--setting-sources user` (never on `claude auth …`), and refuse them when admin policy could route. Evidence: `ai::policy_conflict`
- R7. Start Claude Code children in their own session, never only a process group; only `echoctl ai login`'s child stays in the terminal's foreground. Evidence: `ai::detach`
- R8. During a sign-in, `ai::for_call` fails at once with `SigningIn`, never waits, and live jobs are deferred until it ends. Evidence: `ai::SigningIn`, `livenotes::defer`
- R9. Admit a minutes run only through `notes::admit`, and re-check an automatic run as due right before Claude Code starts. Evidence: `notes::admit`, `notes::auto_due`
- R10. After a run, the app applies only that run's own `actions.new-<run>.json`, never the generic `actions.new.json`, and applying keeps each item's current status. Evidence: `notes::finish_actions`, `calls::apply_actions`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Run the real `claude` binary in a test | Install `ai::fake::install(root)` (a scripted fake Claude Code) | `ai::fake::install` |
