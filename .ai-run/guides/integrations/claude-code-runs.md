# Claude Code Runs

**Project**: Echo · **Stack**: Rust (+ vanilla JavaScript window UI, TypeScript status-line mod), tao + wry + tray-icon, clap, rquickjs · **Scope**: starting Claude Code on AI profiles, strict routing, leases, the fake in tests

**Design**: `.ai-run/guides/design/ai-and-minutes.md` — how it works and why; the rules here win.

## Starting Claude Code

### Rules
- R1. Start Claude Code on call text, for a sign-in check or a sign-in through `ai::claude_command` (or `ai::prepare_claude`), which applies the profile's routing and policy checks; a bare `app::claude_bin()` is used only for `--version` and plugin-management commands (`terminal::claude_new_enough`, `app::update_plugins`, `claude_setup`). Evidence: `crates/echoctl/src/ai.rs::claude_command`
- R2. Pick a call's AI profile with `ai::for_call` / `ai::call_profile_id`: an unreadable `meta.json` or a missing profile is an error, never a fallback to another profile. Evidence: `crates/echoctl/src/ai.rs::call_profile_id`
- R3. When a routing environment-variable family changes, change both `ai::routing_var` (the process environment) and `ai::UNSET_ROUTING` (the login-shell `sed` filter). Evidence: `crates/echoctl/src/ai.rs::UNSET_ROUTING`
- R4. Hold the profile's lease (`ai::lease`, kept by the `Ready` that `ai::ready` returns) for the whole job, so a sign-in waits for it. Evidence: `crates/echoctl/src/ai.rs::ready`

### Constraints
| Do not | Do instead | Why | Evidence |
|---|---|---|---|
| Run the real `claude` binary in a test | Install `ai::fake::install(root)` (a scripted fake Claude Code) | tests must not depend on the developer's account or network | `crates/echoctl/src/ai.rs::fake.install` |
