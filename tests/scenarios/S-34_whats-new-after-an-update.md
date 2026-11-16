---
id: S-34
title: What's new after an update: once per version, skipped versions grouped, Show me, again from ⌘K, Settings and the tray, nothing on a fresh install
run: agent
data: small
cases:
  - tests/cases/lifecycle/TC-014_whats-new-from-the-palette-and-settings.md
  - tests/cases/lifecycle/TC-011_after-an-update-whats-new-shows-once.md
  - tests/cases/lifecycle/TC-016_an-update-from-before-whats-new-shows-this-versions-items.md
  - tests/cases/lifecycle/TC-012_skipped-versions-show-grouped-newest-first.md
  - tests/cases/lifecycle/TC-013_show-me-opens-the-items-place.md
  - tests/cases/lifecycle/TC-015_a-fresh-install-shows-no-whats-new.md
  - tests/cases/lifecycle/TC-017_the-trays-whats-new-opens-the-window-with-it.md
---

# S-34: What's new after an update

**Story:** Sam updates Echo: the first window says what this version brings, once. Someone who skipped versions sees each one's, newest first; "Show me" takes them there; anyone can read it again from ⌘K or Settings. On a fresh install there is nothing to say.

**Data:** `small`: `tests/fixtures/serve.sh up <dir> <port>` (the running version seen, as all test data); each stage sets the last version seen with `tests/fixtures/whats_new_seen.py` after `. <dir>/env`.

**Length:** about 5 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build (by 22:50), the running version seen | the lead (`serve.sh up`) | 1–6 |
| The last version seen, set before each stage | each stage's glue (`tests/fixtures/whats_new_seen.py`) | 2–6 |

## Stages

What's new opens by itself at the window's next load once the last version seen is older, and counts as seen as soon as it shows. So each stage's glue first takes the page away (`about:blank`), then sets the last version seen; the stage's own first load shows it.

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `lifecycle/TC-014_whats-new-from-the-palette-and-settings.md` | — | — (the lead's data: the running version seen) |
| 2 | `lifecycle/TC-011_after-an-update-whats-new-shows-once.md` | — | Navigate to `about:blank`; after `. <dir>/env` run `python3 tests/fixtures/whats_new_seen.py previous` |
| 3 | `lifecycle/TC-016_an-update-from-before-whats-new-shows-this-versions-items.md` | — | Navigate to `about:blank`; after `. <dir>/env` run `python3 tests/fixtures/whats_new_seen.py none` |
| 4 | `lifecycle/TC-012_skipped-versions-show-grouped-newest-first.md` | — | Navigate to `about:blank`; after `. <dir>/env` run `python3 tests/fixtures/whats_new_seen.py 0.0.1` |
| 5 | `lifecycle/TC-013_show-me-opens-the-items-place.md` | — | Navigate to `about:blank`; after `. <dir>/env` run `python3 tests/fixtures/whats_new_seen.py 0.0.1` |
| 6 | `lifecycle/TC-015_a-fresh-install-shows-no-whats-new.md` | — | Navigate to `about:blank`; after `. <dir>/env` run `python3 tests/fixtures/whats_new_seen.py none`; then its Preconditions' CLI (the calls to the trash) before its first load — last: it empties the calls |
| 7 | `lifecycle/TC-017_the-trays-whats-new-opens-the-window-with-it.md` | — | **person** (the tray) |

## Notes
- One data build: the last version seen is the only thing that changes between stages, and the fixture sets it in the run's config folder.
- The app, at its start, marks a data folder with no calls and nothing seen as seen (`whats_new::at_start`, Rust); stage 6 checks the window's side: no calls and nothing seen shows nothing and writes nothing.
