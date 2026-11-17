---
id: S-41
title: "A plugin brings its tabs to Knowledge: alone with Recall off, its hidden tab and badges, deep links, its header strip and kept places, its search, drops and its settings card"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-170_with-recall-off-knowledge-shows-only-a-plugins-tabs-and-goes-with-the-plugin.md
  - tests/cases/plugins/TC-171_a-plugins-knowledge-tabs-show-their-icons-badges-and-a-tab-hidden-until-a-vault-exists.md
  - tests/cases/plugins/TC-172_a-link-to-a-plugins-knowledge-tab-opens-it-with-its-route-back-forward-and-a-reload.md
  - tests/cases/plugins/TC-173_a-plugins-header-strip-and-each-tabs-place-stay-while-moving-between-its-tabs.md
  - tests/cases/plugins/TC-174_knowledges-search-box-searches-the-shown-plugin-tab-or-hides.md
  - tests/cases/plugins/TC-175_files-dropped-on-any-of-a-plugins-knowledge-tabs-go-to-it.md
  - tests/cases/plugins/TC-176_knowledges-gear-opens-its-settings-with-the-plugins-card-under-recalls.md
---

# S-41: A plugin brings its tabs to Knowledge

**Story:** Sam doesn't use Recall but installs a vault plugin. Knowledge appears with only its tabs — Inbox and Vaults,
"!" until Sam adds a vault, then Pages too. Sam opens a page from a link, moves between the plugin's tabs without
losing the place or what was typed into "Ask the vault", searches its pages, drops files on it, and finds its settings
from the gear, under Recall's.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (by 22:50, else `--now`).

**Length:** about 8 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build (Recall off) | the lead (`serve.sh up`) | 1–7 |
| The marketplace `test-plugins` and Vault demo installed | stage 1 (its install precondition) | 1–7 |
| Vault demo's vault | stage 2 (step 2) | 3–7 |

## Prelude

1. Load `{{base_url}}/`.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-170_with-recall-off-knowledge-shows-only-a-plugins-tabs-and-goes-with-the-plugin.md` | — | Run its install lines, reload; skip its teardown |
| 2 | `plugins/TC-171_a-plugins-knowledge-tabs-show-their-icons-badges-and-a-tab-hidden-until-a-vault-exists.md` | 1 | — |
| 3 | `plugins/TC-172_a-link-to-a-plugins-knowledge-tab-opens-it-with-its-route-back-forward-and-a-reload.md` | 2 | — |
| 4 | `plugins/TC-173_a-plugins-header-strip-and-each-tabs-place-stay-while-moving-between-its-tabs.md` | 2 | — |
| 5 | `plugins/TC-174_knowledges-search-box-searches-the-shown-plugin-tab-or-hides.md` | 2 | — |
| 6 | `plugins/TC-175_files-dropped-on-any-of-a-plugins-knowledge-tabs-go-to-it.md` | 2 | Reload the page first |
| 7 | `plugins/TC-176_knowledges-gear-opens-its-settings-with-the-plugins-card-under-recalls.md` | 2 | Its teardown, as the scenario's last: `echoctl plugin remove vault-demo --delete-data`; `echoctl plugin marketplace remove test-plugins` |

## Notes
- One install and one vault serve every stage; only stage 1 switches the plugin off (and on again).
