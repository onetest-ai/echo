# Untrusted Text

**Project**: Echo · **Scope**: headless-run guard, run allowlists, safe Markdown, path checks

**Reference**: `.ai-run/reference/security/untrusted-text.md` — the allowlists, flags, variables, policies and functions
these rules rely on (`ref: <section>`). It changes with the code; these rules change only by a `Rules:` PR.

## Runs, rendering and paths

### Rules
- R1. A minutes run, a project sync and the window's session on a call may call only the CLI commands in their mode's allowlist; the guard refuses any other Bash call. A routine's or Yolo's run has no run guard: it runs as the folder session (R20). Evidence: `guard::PROJECT_STEPS` (ref: Run allowlists)
- R2. A CLI command that changes settings, recording, audio retention or anything outside the run's call refuses inside a run Echo started (every run but a routine acting on its own, R21); the always-on vocabulary, keeping a call's audio and Echo plugin installs and updates refuse in every run. A mail draft is never refused. Evidence: `notes::run_limited`, `notes::refused_in_run` (ref: Refusing inside a run)
- R3. Markdown becomes HTML only through the safe writer: raw HTML shown as text, https links only, no images. Evidence: `minutes::markdown_html` (ref: Safe Markdown)
- R4. A path that comes from a run, a plugin or the page is checked as relative with only normal components before it is joined, then resolved and kept inside its root. Evidence: `grants::resolve` (ref: Path checks)
- R5. Read a run's scope only from the environment Echo set, never from the tool call's input; without that environment the guard allows nothing. Evidence: `guard::RUN_CALL` (ref: Run scope variables)
- R6. The run guard blocks on failure and refuses whatever it can't judge, a panic too; refuse the run when any settings source could switch hooks off. Evidence: `ai::hooks_conflict` (ref: The run guard)
- R7. A minutes run starts with only its fixed tools, no MCP server, no permission prompts and no added directories. Evidence: `notes::args` (ref: Minutes run flags)
- R8. A run reads saved tool results only of its own session, never another session's. Evidence: `guard::own_results` (ref: The run guard)
- R9. A Yolo run (a task's, or an item of Needs you now's) reads only its own thread, and nothing of it changes but its task's state: a call only of that thread and pinned to the profile the run started on, the thread's brief, people and work read-only; never another thread. Evidence: `links::thread_call`, `guard::run_thread` (ref: Yolo scope)
- R10. Rename a run's call folder only at the run's end, and never use a run-given title to find previous minutes. Evidence: `calls::rename_to_title`, `calls::previous_subject` (ref: A minutes run's own call)
- R11. Mermaid on a minutes page runs only at its strict security level. Evidence: `minutes::MERMAID_INIT` (ref: Rendering and content security)
- R12. A minutes run changes nothing outside its call except learned vocabulary for its meeting and pruned frames moved to the trash; the app, not the run, clears old trash. Evidence: `calls::purge_trash` (ref: A minutes run's own call)
- R13. A run on a Claude Code too old for blocking hooks is stopped before it does anything. Evidence: `guard::new_enough` (ref: The run guard)
- R14. Inside a minutes run, the CLI reaches only the run's own call. Evidence: `guard::own_call`
- R15. The minutes render in a sandboxed frame; the window and the minutes page are each served with their own content security policy. Evidence: `minutes::render`, `window::page_reply` (ref: Rendering and content security)
- R16. The served window answers only on localhost, and its IPC only with that run's strict cookie and its own origin. Evidence: `window::serve` (ref: Served window)
- R17. Secrets are written readable only by the user; a profile's values reach Claude Code only as environment variables, never in a script's text. Evidence: `ai::PASS` (ref: Secrets)
- R18. An update is installed only verified, never while a call records or before the window's notes are saved. Evidence: `update::commit` (ref: Updates)
- R19. The view tag goes only to Echo's own terminals, never to runs, plugins or other sessions. Evidence: `view::SESSION_VAR` (ref: The view tag)
- R20. The folder session, a routine and a Yolo run get their context and permissions from one builder, and the user's one choice, "Claude acts on its own", sets all three. A run with no tools keeps none whatever the choice; minutes, project sync, the window's session on a call and live notes keep their own runs and guards. Evidence: `folder_args::session`, `claudejob::run` (ref: One builder for the folder session, routines and Yolo)
- R21. Acting on its own: no permission prompts; a routine's run has the CLI's limits for runs lifted, as the folder session has none, and a routine or Yolo may send; Yolo keeps its own task's and thread's limits (R9); Echo plugin installs stay refused in every run (R2). Evidence: `notes::run_limited`, `m365::not_in_run`
- R22. Preparing and asking (the choice off): Echo's reads and its own data's edits run without a prompt; a Microsoft 365 send is refused, pointing to a draft; a plugin command's host half is read-only and only read-only scripts run; a thread's work refuses destructive and structural changes and a task's run settings; anything else that would ask isn't done, and the run says why. Evidence: `folder_args::FREE_COMMANDS`, `cli::project::refused_when_asking` (ref: Preparing and asking)
- R23. Every change and send a routine's or Yolo's run makes through Echo's CLI or a plugin is noted and given in its outcome; its edits of a thread's work are recorded as the run's and applied as the user's would be, and a task setting a run set shows as the run's until the user sets it. A Claude run started inside one doesn't carry its markers unless Echo sets them; a plugin script inside one does. Evidence: `notes::run_actions`, `Node::by_run` (ref: Run actions and markers)

### Constraints
| Do not | Do instead | Evidence |
|---|---|---|
| Let a minutes run write Echo's own call files or instruction files | Write only the minutes and its own action list, applied through the CLI (ref: A minutes run's own call) | `guard::own_call` |
