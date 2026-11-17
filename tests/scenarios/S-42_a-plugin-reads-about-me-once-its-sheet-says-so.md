---
id: S-42
title: "A plugin reads About me once its sheet says so: refused before, an update that asks again, then only the parts it asks"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-162_a-plugin-without-me-on-its-sheet-cant-read-about-me-and-an-update-that-adds-it-asks-again.md
  - tests/cases/plugins/TC-163_a-plugin-reads-about-me-in-the-parts-it-asks-never-how-you-work-unless-asked-and-nothing-from-the-template.md
---

# S-42: A plugin reads About me once its sheet says so

**Story:** Sam installs a coaching plugin. Its first version can't read his About me; its update asks for it, and
Echo's sheet says what that means. Once Sam agrees, it greets him by name and writes the way he likes — but how he
works stays out unless it asks, and with no About me of his own it gets nothing.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (by 22:50, else `--now`);
every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`.

**Length:** about 3 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build | the lead (`serve.sh up`) | 1–2 |
| The copy `$C` of the test marketplace and Me demo installed, updated to 0.2.0 | stage 1 | 1–2 (stage 2's teardown removes them) |

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-162_a-plugin-without-me-on-its-sheet-cant-read-about-me-and-an-update-that-adds-it-asks-again.md` | — | Run its precondition lines as written; skip its teardown: keep Me demo 0.2.0 and `$C` for stage 2 |
| 2 | `plugins/TC-163_a-plugin-reads-about-me-in-the-parts-it-asks-never-how-you-work-unless-asked-and-nothing-from-the-template.md` | 1 | Me demo 0.2.0 is installed: skip its install precondition |

## Notes
- CLI only: no window step. Stage 2 rewrites About me, so it runs last.
