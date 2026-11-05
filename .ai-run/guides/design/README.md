# Design notes

Why Echo works the way it does, per area. The guides hold the **rules**; these notes hold the **background**
(how it works, decisions and their reasons, files on disk, known gaps). Read the rules guide first; open a design note
when a change touches how an area works, not just what a rule says. Where a note and the code disagree, the code wins
— fix the note.

| Area | Design note | Rules guide(s) | Longer plan / measurements |
|---|---|---|---|
| Whole system | — | `../architecture/architecture.md` (overview, data layout, flows, patterns) | — |
| Recording, capture, transcription, memos | `recording.md` | `../capture/capture-and-transcription.md` | `docs/windows-todo.md` |
| The window, tray, `echo://` links, Calendar | `window.md` | `../window/window-ui.md` | — |
| AI profiles, sign-in, live notes, minutes runs, updates | `ai-and-minutes.md` | `../integrations/claude-code-runs.md`, `../security/untrusted-text.md` | `docs/live-notes-design.md` |
| Threads, clients, work ledger, people, tasks | `threads.md` | `../development/threads-and-ledger.md` | `docs/projects-design.md` |
| Recall (the Brain) | `recall.md` | `../data/recall-store.md` | `docs/brain-design.md` |
| Echo plugins | `plugins.md` | `../plugins/echo-plugins.md`, `../plugins/plugin-authoring.md` | `docs/plugins-design.md`, `docs/obsidian-design.md` |
| Claude Code plugin, sessions, `echoctl m365` | `claude-code.md` | `../plugins/claude-code-plugin.md` | `docs/install.md` (users) |

Keeping them current: when a change alters how an area works (a new file on disk, a moved responsibility, a reversed
decision), update that area's note in the same change. Keep each note under ~300 lines; cut history, keep reasons.
