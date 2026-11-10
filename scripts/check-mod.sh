#!/usr/bin/env bash
# Validate, test and type-check the Echo mod (plugin/hooks) the way the engine reads it, and test
# the window's notes saving and calendar dates.
# Type-checks against the engine's own declarations: the copy it lays beside a hot-reloaded mod
# (plugin/.claude-plugin/types) if present, else the one the plugin-authoring skill wrote.
set -euo pipefail
cd "$(dirname "$0")/.."
# Source files under 1,500 lines, today's large ones at their ceilings (architecture.md, Structure and boundaries R9).
scripts/size-check.sh
claude plugin validate plugin
claude plugin test plugin
# The Echo window's notes saving (NotesSync, crates/echoctl/ui/app/02-notes-sync.js), as it ships.
node crates/echoctl/ui/app-parses.test.mjs
node crates/echoctl/ui/notes-sync.test.mjs
# The calendar's days and hours across the days the clocks change.
node crates/echoctl/ui/calendar-dates.test.mjs
# The window's workbench: activities, call tabs, calendar actions, and where panel views go.
node crates/echoctl/ui/workbench.test.mjs
# My notes as moments: note lines of my-notes.md read and written (NoteDoc).
node crates/echoctl/ui/my-notes.test.mjs
# The ⌘K palette's ranking (Palette).
node crates/echoctl/ui/palette.test.mjs
# How a project is drawn (identicons) and its stages (ProjectArt).
node crates/echoctl/ui/projects.test.mjs
# The org chart's layout: governance band, workstream columns, their leads (OrgLayout).
node crates/echoctl/ui/org-chart.test.mjs
# How Send to Claude ranks the skills for a task (rankSkills).
node crates/echoctl/ui/skill-fit.test.mjs
# Send to Claude's Escape: the skill list folds, then the dialog closes (ScKeys).
node crates/echoctl/ui/send-claude.test.mjs
# A client page's tabs: a count only when there is something to count (clientTabs).
node crates/echoctl/ui/client-tabs.test.mjs
# Where a Work board column's menu goes, over the board (colMenuSpot).
node crates/echoctl/ui/column-menu.test.mjs
# Opening a thread loads its work tree on every tab (loadProject).
node crates/echoctl/ui/thread-load.test.mjs
# History's words for the user's own ledger edits (humanHistory).
node crates/echoctl/ui/human-history.test.mjs
# My notes' timeline with live notes among the user's own (LiveTimeline).
node crates/echoctl/ui/live-timeline.test.mjs
# Which transcript turn a moment (a minutes chip, a note's time) opens on (jumpTurn).
node crates/echoctl/ui/transcript-jump.test.mjs
# The Minutes tab without Claude Code or Echo's plugin: what to do, in the window (minutesNeeds).
node crates/echoctl/ui/minutes-needs.test.mjs
# What the poll redraws for: the Inbox's list too (fingerprint, #115, #121).
node crates/echoctl/ui/fingerprint.test.mjs
# The Work tab's statuses, late dates, waiting chips and filters (the work-status block).
node crates/echoctl/ui/work-status.test.mjs
# Settings search: sections by their names and their settings' own labels (SettingsSearch).
node crates/echoctl/ui/settings-search.test.mjs
# Settings → Layout's "Echo's own title bar": a Windows-only row, absent (and unsearchable) elsewhere (#76).
node crates/echoctl/ui/own-title-bar.test.mjs
node crates/echoctl/ui/layout-save.test.mjs
# Today's widgets in rows, in its layout's order (todayRows, #323).
node crates/echoctl/ui/today-rows.test.mjs
# Today's columns from its width and its rows as lines (todayCols / todayLines, #328).
node crates/echoctl/ui/today-layout.test.mjs
# Your day as where you'll be: overlaps folded, your work in gaps, no reply / maybe (agendaRows).
node crates/echoctl/ui/today-agenda.test.mjs
# Your day as a calendar and a strip: meetings side by side in lanes, the hours shown (dayLanes, dayWindow).
node crates/echoctl/ui/today-calendar.test.mjs
# Today's edit mode: where a dragged widget goes over another (dropSide).
node crates/echoctl/ui/today-edit.test.mjs
# The day's choices: overlaps you accepted, what Echo suggests, invites to answer (dayChoices).
node crates/echoctl/ui/today-choices.test.mjs
# The virtual model for the functional cases (#275): scripts, answers, streaming, log.
python3 tests/fixtures/model_server_test.py
types="plugin/.claude-plugin/types/claude-code/index.d.ts"
if [[ ! -f "$types" ]]; then
  types=$(ls -t /private/tmp/claude-*/bundled-skills/*/*/plugin-authoring/types/claude-code.d.ts 2>/dev/null | head -1 || true)
fi
if [[ -z "${types:-}" ]]; then
  echo "skip tsc: no engine type declarations found (load the plugin-authoring skill once)"; exit 0
fi
tmp=$(mktemp -d)
cat > "$tmp/tsconfig.json" <<JSON
{ "compilerOptions": { "target": "es2023", "lib": ["es2023"], "types": [], "module": "esnext",
  "moduleResolution": "bundler", "strict": true, "noUncheckedIndexedAccess": true, "noEmit": true,
  "skipLibCheck": true, "jsx": "react", "jsxFactory": "h", "jsxFragmentFactory": "Fragment" },
  "files": ["$types"], "include": ["$PWD/plugin/hooks", "$PWD/plugin/types"] }
JSON
npx -y -p typescript@5.6.3 tsc -p "$tmp/tsconfig.json"
echo "tsc: clean"
