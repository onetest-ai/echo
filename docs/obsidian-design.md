# Obsidian vaults — plan (draft, 2026-11-03)

An Echo plugin (v4: it carries its own code; Echo adds only primitives) in `onetest-ai/echo-plugins/plugins/obsidian`.
Screens: the Echo Workbench canvas, row "Plugins — Obsidian".

## What the user decided

- **Connect the vaults people already have** ("подключение внешних волтов которые уже есть у людей своих"). One or more;
  a vault may be one engagement's. Echo never makes a vault of its own, never exports the Brain into one.
- **A vault is human-readable; the Brain is not** — the Brain is for agents, a vault for people and agents both. So Echo
  shows a vault the way Obsidian does: an **Obsidian activity** in the activity bar with a file browser, the note, and a
  graph built from wikilinks or tags.
- **Connecting is short**: connect → Echo analyses the vault → it writes a **description for Claude** (what the vault is
  and what Claude can do with it) → the user corrects the description → done. No mapping review, no write previews.
- A vault holds knowledge, not operational notes.
- **The vault and the Brain stay apart** ("волт отдельно — брейн отдельно", 2026-11-03): a vault is never indexed into the Brain;
  Claude reads it directly (the skill, the folder). The Brain keeps calls, mail, channels and thread folders.

## 1. Connect and analyse

Settings → From plugins → Obsidian → + Vault (the system's folder picker; the vaults Obsidian knows are offered from its
`obsidian.json`).

The analysis:
1. **Survey**, plugin code with no model: folders and counts, frontmatter keys, tags, link density, hubs / MOCs, daily
   notes and templates folders, community plugins, notes whose names match Echo's clients, threads and people.
2. **One Claude run** on the default AI profile, with no tools. It gets the survey plus the first lines of about 30
   sampled notes, never from folders the survey flags as journal or private.
3. It writes the **description**, plain Markdown in the vault's own terms:
   - **What it is** (engagement, personal knowledge, team wiki).
   - **Where things are**.
   - **Conventions**: frontmatter, links, tags, how a person, decision or overview note looks.
   - **What you can do**: answer from it and cite notes; add knowledge in its own shape; ask before changing someone's
     note.
   - **Leave alone**: journal, templates, archive.
   - **Belongs to**: which folders or tags are which client or thread.

The description is saved as `<plugin store>/vaults/<id>/description.md`, edited in place on the vault's Settings card,
and is the user's from then on. Analyse again offers a new version beside it and never overwrites it.

## 2. How a vault tells Claude it is there

Not every description in every session: with twenty vaults that is thousands of tokens about vaults the session
doesn't need. The way Claude Code handles skills instead: a short line always, the rest on demand.

- **The analysis writes two parts**:
  - **When Claude looks here**: one line of 250 characters or less, e.g. "Acme and Globex delivery: their people, decisions
    and how the team works. Look here when a question is about who's who, what was decided or how things are done for
    these clients."
  - **The description**: the full text.
  Both are the user's to edit; the line comes first on the card, because it decides whether Claude goes there.
- **One skill for all vaults, in three levels** (not a skill per vault: the same body copied 100 times, and 100
  descriptions in every session):
  1. **The skill `obsidian-vaults`**, always listed. Its `description` is built from the vaults: "Your Obsidian notes:
     12 vaults — Acme, Globex, Initech… and 3 personal. Look here when the answer may be in the user's notes." Its body
     is written once: how to work with a vault (wikilinks, tags, MOCs, opening a note with `obsidian://open`) and how to
     find the right one.
  2. **The catalog**, `<plugin store>/vaults.md`: each vault's "when Claude looks here" line and its path, grouped by
     client. Read when the skill applies. For many vaults it is searchable: the plugin's `find` command, also reachable
     through `echoctl plugin call obsidian find`, allowed in the sessions that see vaults.
  3. **A vault's description**, `vaults/<id>/description.md`. Read once a vault is chosen.
  The skill sits in one generated Claude Code plugin, `<plugin store>/claude-plugin/`, rebuilt when vaults change.
  Echo's sessions get it with `--plugin-dir`, as minutes runs get Echo's own plugin.
- **Pointed to from the view**: when a thread or client is open, the `<echo_view>` hook names its vaults directly: "this thread has the vault
  Team wiki: ~/Vaults/delivery-wiki — read its description at …". No catalog step; the main path for everyday questions.
- **Who sees what**:

  | Session | Vaults |
  |---|---|
  | Claude · Echo folder | all: skills, read the folders, write under the user's own permissions |
  | Claude · this call | the vaults of the call's threads and client, read-only (allow rules in its guard's ask mode) |
  | Scheduler routines | all, read-only (`RUN_MODE=routine` reads the vault folders too) |
  | Minutes, thread sync, live notes | none (their guards stay as they are) |


## 3. The Obsidian activity (people browse it)

The plugin adds an activity, ◆ Obsidian, under Brain. Layout as Obsidian's:
- **Left**: the vault switcher, a search box, **Files | Tags** (the folder tree with counts, or the tag tree). Skipped
  folders are greyed. A note Echo or Claude added to has a violet dot.
- **Middle**: tabs of open notes plus Graph.
  - A note shows its path, its **Properties** (frontmatter) and rendered Markdown through Echo's safe writer.
  - Wikilinks open the linked note in place; Open in Obsidian ↗ (`obsidian://open`).
  - Read-only in Echo: editing happens in Obsidian.
- **Right**: the note's **local graph** (Links | Tags), **backlinks** with their line, and **In Echo** (the same person,
  client or thread in Echo, with its calls and tasks).
- **Graph tab**: the whole vault, by Links | Tags | Both. Colour by Folder | Client | Tag; orphans hidden.
  - Find a note or tag; hover lights the neighbours; click opens the note on the right.
  - Drawn by Echo's GraphView (Sigma.js, already vendored).

## 4. What Echo adds (primitives)

| Primitive | For |
|---|---|
| `ctx.activities.add({id, label, icon, render, panels})` | the Obsidian activity (Echo's own `ACTIVITIES` take plugin entries) |
| `ctx.ui.graph` (GraphView over graphology) and `ctx.markdown` in the client half | the graph, the note |
| `ctx.fs` scoped to the vault folders the user picked (read; write none — Claude writes, under its own permissions) | survey, browsing, reading |
| `ctx.echo.read(kind)` — clients, threads, people | the survey's matching, "In Echo" |
| `ctx.sessions.plugin(dir)` (one generated Claude Code plugin with the `obsidian-vaults` skill, given to Echo's sessions with `--plugin-dir`), `ctx.sessions.read(paths, {write})`, `ctx.view.hint(scope, text)` | the skill and its catalog, vault access, the pointer from the open thread / client |
| `ctx.claude.run` (exists) | the analysis |

## Phases

| Phase | Scope |
|---|---|
| A | Connect, analyse, the line + description (Settings card), the `obsidian-vaults` skill + catalog in Claude's sessions, read access, the view's pointer |
| B | The Obsidian activity: file browser, note, local graph, backlinks, graph tab |
