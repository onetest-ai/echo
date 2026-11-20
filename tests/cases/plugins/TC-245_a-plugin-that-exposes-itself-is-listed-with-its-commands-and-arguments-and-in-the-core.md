---
id: TC-245
title: A plugin that exposes itself is listed with its commands and their arguments, and named in Claude's core; one that doesn't, or is switched off, isn't
priority: high
type: functional
requirements: [F-PLUG-156]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, offers, cli]
---

# TC-245: What a plugin offers, as others see it

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/make-data.sh <dir>`
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; E=target/debug/echoctl`
- A made-up local marketplace `acme-offers` with five plugins: **Acme vaults** offers two commands, **Acme asker** asks to call other plugins, **Acme nosy** doesn't, **Acme quiet** offers nothing, **Acme guide** offers no commands but gives Claude a guide:

```bash
M="$D/market-offers"; for p in acme-vaults acme-asker acme-nosy acme-quiet acme-guide/agent; do mkdir -p "$M/plugins/$p"; done
echo '{"name":"acme-offers","plugins":[{"id":"acme-vaults","source":"./plugins/acme-vaults"},{"id":"acme-asker","source":"./plugins/acme-asker"},{"id":"acme-nosy","source":"./plugins/acme-nosy"},{"id":"acme-quiet","source":"./plugins/acme-quiet"},{"id":"acme-guide","source":"./plugins/acme-guide"}]}' > "$M/echo-marketplace.json"
echo '{"id":"acme-guide","name":"Acme guide","version":"0.1.0","summary":"A made-up plugin for tests","agent":{"summary":"Staffing of Acme projects"}}' > "$M/plugins/acme-guide/echo-plugin.json"
printf '# Acme guide\n' > "$M/plugins/acme-guide/agent/CLAUDE.md"
echo '{"id":"acme-vaults","name":"Acme vaults","version":"0.1.0","summary":"Search and notes across the Acme vaults","main":"main.js"}' > "$M/plugins/acme-vaults/echo-plugin.json"
cat > "$M/plugins/acme-vaults/main.js" <<'JS'
export function apply(ctx) {
  const api = ctx.expose('A text Echo does not read')
  api.command('search', ({query, limit = 10}, how) => ({query, limit, from: how.from || null}), {description: 'Find notes by words, when a fact from a vault is needed', reads: true, args: {query: 'the words to find'}})
  api.command('note', ({path, text}) => 'written ' + path, {description: 'Write a note into a vault'})
  ctx.command('secret', () => 'mine')
}
JS
echo '{"id":"acme-asker","name":"Acme asker","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js","uses":{"plugins":true}}' > "$M/plugins/acme-asker/echo-plugin.json"
echo "export function apply(ctx) { ctx.command('ask', (a) => ctx.plugins.call(a.id, a.name, a.args)); ctx.command('list', () => ctx.plugins.list()) }" > "$M/plugins/acme-asker/main.js"
echo '{"id":"acme-nosy","name":"Acme nosy","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js"}' > "$M/plugins/acme-nosy/echo-plugin.json"
echo "export function apply(ctx) { ctx.command('ask', (a) => ctx.plugins.call(a.id, a.name, a.args)) }" > "$M/plugins/acme-nosy/main.js"
echo '{"id":"acme-quiet","name":"Acme quiet","version":"0.1.0","summary":"A made-up plugin for tests","main":"main.js"}' > "$M/plugins/acme-quiet/echo-plugin.json"
echo "export function apply(ctx) { ctx.command('ping', () => 'pong') }" > "$M/plugins/acme-quiet/main.js"
$E plugin marketplace add "$M" --name acme-offers
```

**Changes:** `<config dir>/plugins/` (the marketplace, the plugins, `offers/`), the plugins' stores

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install acme-vaults --yes`; `$E plugin install acme-quiet --yes` | Both install |
| 2 | `$E plugin commands` | One plugin: `[{"description": "Search and notes across the Acme vaults", "id": "acme-vaults", "name": "Acme vaults"}]` — its manifest's summary, never the text given to `ctx.expose` ("A text Echo does not read"); Acme quiet isn't listed |
| 3 | `$E plugin commands acme-vaults` | Its `commands` are exactly `search` (`"reads": true`, description `Find notes by words, when a fact from a vault is needed`) and `note` (`"reads": false`); `secret` isn't among them |
| 4 | `$E plugin commands acme-vaults search`; then `$E plugin commands acme-vaults secret` | The first: `args` are `query` (`"optional": false`, `"about": "the words to find"`) and `limit` (`"optional": true`, `"default": "10"`); the second fails: `acme-vaults offers no command “secret”` |
| 5 | `$E core \| sed -n '/^## Plugins$/,$p'` | The plugins' table (`\| Plugin \| Id \| What it gives \|`), its line "ask `echo_plugins`' plugin(id) for its commands, skills and guide", and the row ``\| Acme vaults \| `acme-vaults` \| Search and notes across the Acme vaults \|``; no row for `acme-quiet` |
| 6 | `$E plugin disable acme-vaults`; `$E plugin commands`; `$E core \| grep -c acme-vaults` | `[]`; `0` — switched off, it is offered to nobody |
| 7 | `$E plugin enable acme-vaults`; `$E plugin commands \| grep -c acme-vaults` | `1`: listed again |

## Teardown
- `$E plugin remove acme-vaults`; `$E plugin remove acme-quiet` (TC-246 and TC-247 install their own when run alone)
