# Claude Code Runs

**Project**: Echo · **Scope**: starting Claude Code on AI profiles, strict routing, leases, the fake in tests

**Reference**: `.ai-run/reference/integrations/claude-code-runs.md` — the functions, flags and files these rules rely on
(`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Starting Claude Code

### Rules
- R1. Start Claude Code only through Echo's command builder; the bare binary only for its version and plugin management. Evidence: `ai::claude_command` (ref: Starting Claude Code)
- R2. A call's AI profile is the one pinned to it: an unreadable `meta.json` or a missing profile is an error, never a fallback to another profile. Evidence: `ai::call_profile_id` (ref: Profiles)
- R3. When a routing environment-variable family changes, change every place that lists them. Evidence: `ai::UNSET_ROUTING` (ref: Routing variables)
- R4. Hold the profile's lease for the whole job. Evidence: `ai::ready` (ref: Profiles)
- R5. Pin a call's AI profile into `meta.json` at recording start. Evidence: `ai::pin`
- R6. `local` / `claude` jobs read only the user's settings, and are refused when admin policy could route them. Evidence: `ai::policy_conflict` (ref: Starting Claude Code)
- R7. Start Claude Code children in their own session, never only a process group; only the sign-in's child stays in the terminal's foreground. Evidence: `ai::detach` (ref: Starting Claude Code)
- R8. During a sign-in, a job asking for a profile fails at once, never waits, and live jobs are deferred until it ends. Evidence: `ai::SigningIn`, `livenotes::defer` (ref: Profiles)
- R9. Admit a minutes run only through `notes::admit`, and re-check an automatic run as due right before Claude Code starts. Evidence: `notes::admit`, `notes::auto_due`
- R10. After a run, the app applies only that run's own action list, never another's, and applying keeps each item's current status. Evidence: `notes::finish_actions`, `calls::apply_actions` (ref: Minutes runs)
- R11. A `claude` profile signed in to another account than the one recorded runs no job until the user accepts it. Evidence: `ai::accept_account`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Run the real `claude` binary in a test | Install the scripted fake Claude Code (ref: Tests) | `ai::fake::install` |
