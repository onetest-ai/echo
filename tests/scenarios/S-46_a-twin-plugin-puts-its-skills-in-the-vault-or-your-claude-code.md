---
id: S-46
title: "A twin plugin puts its skills in the vault's own Claude Code project, or in yours on a click — never Claude Code's settings"
run: agent
data: standard
cases:
  - tests/cases/plugins/TC-201_a-plugin-writes-its-vaults-own-skill-not-its-settings-and-adds-a-skill-to-your-claude-code-on-your-click.md
---

# S-46: A twin plugin puts its skills in the vault or in your Claude Code

**Story:** Sam gives a notes plugin his Acme vault. Its twin's skill goes into the vault's own Claude Code project, so
Claude in that folder loads it; the plugin can't write the vault's Claude Code settings. When Sam wants the skill
everywhere, he clicks "Add to Claude Code" and it lands in his own Claude Code — never by itself, never over a skill of
his, and gone again with the plugin.

**Data:** `standard`: `tests/fixtures/serve.sh up <dir> <port> --set standard`; every `echoctl` command is
`target/debug/echoctl` (built right before) after `. <dir>/env`.

**Length:** about 3 minutes.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| The `standard` data build | the lead (`serve.sh up`) | 1 |
| The marketplace `acme-local` with Acme twin, given `$D/vault` | stage 1 (its preconditions and step 1) | 1 |

## Prelude
1. Open a new tab for this scenario; load `{{base_url}}/`.

## Stages

| # | Case | Depends on | Before it |
|---|------|------------|-----------|
| 1 | `plugins/TC-201_a-plugin-writes-its-vaults-own-skill-not-its-settings-and-adds-a-skill-to-your-claude-code-on-your-click.md` | — | Run its precondition lines as written; its step 8 and teardown remove the plugin and `acme-local` |

## Notes
- The test data's `env` sets `CLAUDE_CONFIG_DIR` to `<dir>/claude-config`: the skill never reaches this computer's `~/.claude`. Close the tab and `serve.sh down <dir>` at the end.
