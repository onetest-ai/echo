# Claude Code Runs

**Project**: Echo · **Scope**: starting Claude Code on AI profiles, strict routing, leases, the fake in tests

**Reference**: `.ai-run/reference/integrations/claude-code-runs.md` — the functions, flags and files these rules rely on
(`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Starting Claude Code

### Rules
- R1. Start Claude Code only through Echo's command builder; the bare binary only for its version and plugin management, and a wrapper profile's command (R12) never for plugin management, its version only as R13 and R14 ask. Evidence: `ai::claude_command` (ref: Starting Claude Code)
- R2. A call's AI profile is the one pinned to it: an unreadable `meta.json` or a missing profile is an error, never a fallback to another profile. Evidence: `ai::call_profile_id` (ref: Profiles)
- R3. When a routing environment-variable family changes, change every place that lists them. Evidence: `ai::UNSET_ROUTING` (ref: Routing variables)
- R4. Hold the profile's lease for the whole job. Evidence: `ai::ready` (ref: Profiles)
- R5. Pin a call's AI profile into `meta.json` at recording start. Evidence: `ai::pin`
- R6. `local` / `claude` jobs read only the user's settings, and are refused when admin policy could route them; a plugin's run in a folder also reads that folder's project settings, and is refused when they could route it. Evidence: `ai::policy_conflict`, `ai::routes` (ref: Starting Claude Code)
- R7. Start Claude Code children in their own session, never only a process group; only the sign-in's child stays in the terminal's foreground. Evidence: `ai::detach` (ref: Starting Claude Code)
- R8. During a sign-in, a job asking for a profile fails at once, never waits, and live jobs are deferred until it ends. Evidence: `ai::SigningIn`, `livenotes::defer` (ref: Profiles)
- R9. Admit a minutes run only through `notes::admit`, and re-check an automatic run as due right before Claude Code starts. Evidence: `notes::admit`, `notes::auto_due`
- R10. After a run, the app applies only that run's own action list, never another's, and applying keeps each item's current status. Evidence: `notes::finish_actions`, `calls::apply_actions` (ref: Minutes runs)
- R11. A `claude` profile signed in to another account than the one recorded runs no job until the user accepts it. Evidence: `ai::accept_account`
- R12. A wrapper profile starts a command the user installed in place of `claude`. It is a profile of its own, never `system`, and runs on the user's own setup as `system` does, with the hook check but never the routing-policy check. Its command is the absolute path Echo resolved and showed when the profile was added (found as a run's login shell finds it, links followed), stored, passed as a program and never in a script's text; every start on the profile uses that path. Evidence: `ai::prepare_claude`
- R13. A wrapper profile is added only by the user, in the window or the CLI, by typing its command or taking a plugin's offer (echo-plugins R24); it is added only if the resolved path answers `--version` as Claude Code and lies outside Echo's data and config folders and every plugin's package and store. A run never sets one, and no plugin makes it the default or chooses it for a call, a thread, a routine or a job: a plugin's job runs on it only when the user's default or the thread's pin names it. Its command changes only when the user accepts the new one on the profile. The profile shows the path, the plugin that offered it, and that this program sees and can change everything Echo sends to Claude, call transcripts and Echo's checks included. Evidence: `ai::ProfileChange`
- R14. Through a wrapper, every run carries Echo's guard, refusing every tool in a run that has none, and gets its text from a call, a meeting, a model or a plugin only on stdin (in a terminal, typed in), never on the command line but for the paths Echo made. No text reaches it and no tool runs until Echo has proved, for that very run, that its own settings are in force and that Claude Code started new enough for the guard, with the permission mode, tools and MCP servers Echo set; Echo stops a run that doesn't match, saying why. How the proof works: ref Wrapper profiles. Evidence: `ai::wrapper_ready`, `guard::checked`
- R15. A run waiting on its wrapper (a sign-in, a network check) ends within its job's own time limit, naming the command; it is never moved to another profile. Evidence: `ai::wait_output`

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Run the real `claude` binary in a test | Install the scripted fake Claude Code (ref: Tests) | `ai::fake::install` |
