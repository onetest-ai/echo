---
id: S-37
title: "Ask a plugin's folder in a Claude session: on the user's click, as that folder's own project, one per plugin in its own tab, found again, a new conversation, what is refused, the plugin off"
run: agent
data: small
cases:
  - tests/cases/plugins/TC-140_a-plugins-claude-session-opens-in-its-folder-as-that-folders-own-project-with-the-windows-permission-setting.md
  - tests/cases/plugins/TC-141_a-plugins-claude-session-opens-only-on-the-users-click.md
  - tests/cases/plugins/TC-142_a-plugins-claude-session-runs-once-asked-again-it-is-shown-in-another-folder-it-throws.md
  - tests/cases/plugins/TC-143_each-plugins-claude-session-has-its-own-tab.md
  - tests/cases/plugins/TC-144_a-plugins-claude-session-is-refused-before-it-starts-when-its-request-doesnt-fit.md
  - tests/cases/plugins/TC-146_a-plugins-claude-session-found-running-after-the-window-reopens-resumes-on-its-profile.md
  - tests/cases/plugins/TC-147_a-new-conversation-in-a-plugins-claude-session-from-the-actions-palette.md
  - tests/cases/plugins/TC-145_a-plugins-claude-session-runs-on-when-the-plugin-is-switched-off-but-doesnt-start-again.md
---

# S-37: Ask a plugin's folder in a Claude session

**Story:** Sam gave a notes plugin his made-up vault. In its screen he clicks "Ask the vault": Claude Code opens in Echo's panel, in that folder, as he set Claude in the window. It opens only on his click, once per plugin; a second plugin's session gets a tab of its own; after the window reopens both are there, and Resume keeps the profile each ran on. He starts a new conversation from ⌘K. What doesn't fit is refused before Claude Code starts, and switching the plugin off leaves his open session running.

**Data:** `small`: `tests/fixtures/serve.sh up <dir> <port>`; Claude Code is the case's own fake `claude` (a session that writes down how it was started, then waits), never a real account.

**Length:** about 10 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `small` data build | the lead (`serve.sh up`) | all |
| The profile `acme-work`, the session-keeping fake `claude`, the folder `$D/vault`, the marketplace `acme-ask` and Acme ask installed with `$D/vault` given to it; the IPC helpers | stage 1 (TC-140's preconditions and step 1) | 1–8 (removed by stage 8's teardown) |
| Acme ask B, installed with `$D/vault` | stage 4 (its preconditions) | 4 (removed by its teardown) |

## Prelude

1. Load `{{base_url}}/`.

## Stages

Every stage after 1 runs in Bash with TC-140's prelude (`D`, `E`, `B` and the helpers) set again: each Bash call is a fresh shell.

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-140_a-plugins-claude-session-opens-in-its-folder-as-that-folders-own-project-with-the-windows-permission-setting.md` | — | Run its precondition lines as written. Skip its teardown: Acme ask, `$D/vault`, `acme-work` and the fake `claude` stay for stages 2–8 |
| 2 | `plugins/TC-141_a-plugins-claude-session-opens-only-on-the-users-click.md` | 1 | Skip TC-140's precondition lines and its step 1 (stage 1's). Of its teardown run only `close` |
| 3 | `plugins/TC-142_a-plugins-claude-session-runs-once-asked-again-it-is-shown-in-another-folder-it-throws.md` | 1 | As for stage 2 |
| 4 | `plugins/TC-143_each-plugins-claude-session-has-its-own-tab.md` | 1 | As for stage 2, then its own precondition lines for Acme ask B (with `M="$D/market-ask"`). Of its teardown skip TC-140's |
| 5 | `plugins/TC-144_a-plugins-claude-session-is-refused-before-it-starts-when-its-request-doesnt-fit.md` | 1 | As for stage 2. Skip its teardown |
| 6 | `plugins/TC-146_a-plugins-claude-session-found-running-after-the-window-reopens-resumes-on-its-profile.md` | 1 | As for stage 2 |
| 7 | `plugins/TC-147_a-new-conversation-in-a-plugins-claude-session-from-the-actions-palette.md` | 1 | As for stage 2 |
| 8 | `plugins/TC-145_a-plugins-claude-session-runs-on-when-the-plugin-is-switched-off-but-doesnt-start-again.md` | 1 | As for stage 2, but its teardown in full: `close`, then TC-140's (Acme ask, `acme-ask`, `acme-work` and `$D/vault` go, the data's fake `claude` back) |

## Notes
- A served window doesn't run the app's IPC answers (the page's CSP), so the cases reload the page to see a session as the app has it; a reload is also the window reopening (sessions found running).
- Nothing is fetched from the network: the marketplaces are local folders in the run's scratch folder.
