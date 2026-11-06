# {{vault.name}}

{{vault.owner}}'s knowledge vault: plain Markdown in git, made with Echo's starter.

## Getting started

1. **Tell it who you are.** Fill in `knowledge/profile/DIGITAL_PROFILE.md` — your role, what you work on, how you
   write. Everything Claude answers as you starts there.
2. **Add what you know.** Drop documents, exports or transcripts into `raw_inbox/`, then ask Claude:
   `/process-inbox`. Each becomes a page in `knowledge/`, listed in `knowledge/CATALOG.md`.
   Or say it directly: `/twin remember that …`.
3. **Ask it.** `/twin what did we decide about …?` — Claude reads the index, opens the few pages that match and
   answers with the files it used.
4. **Meetings come in by themselves.** When Echo writes the minutes of a call, it saves them to `knowledge/notes/`
   and adds them to the index (Echo → Knowledge → this vault → Settings → Meeting minutes).

## Where to open it

| With | How |
|---|---|
| Echo | Knowledge → {{vault.name}}: pages, graph, editing, Ask |
| Obsidian | Open folder as vault → choose `knowledge/` |
| Claude Code | `cd` here and run `claude`; the `twin` and `process-inbox` skills are in `.claude/skills/` |
| Any editor or agent | It is Markdown; `AGENTS.md` tells agents the rules |

## Folders

| Folder | What goes there |
|---|---|
| `knowledge/` | the pages |
| `raw_inbox/` | sources, exactly as they came; never edited |
| `.claude/skills/` | the skills that keep the vault |
| `log.md` | what changed, one line each |
