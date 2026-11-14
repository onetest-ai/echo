---
id: TC-040
title: Discover's Featured follow the marketplace's `featured` order, not names; `echoctl plugin marketplace list` lists each marketplace's plugins
priority: medium
type: functional
requirements: [F-PLUG-02, F-PLUG-10]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, discover, cli]
---

# TC-040: Discover follows a marketplace's featured order; the CLI lists its plugins

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard`; the window served at `{{base_url}}`, `{{base_url}}/` loaded first; CLI steps after `. <dir>/env`; the browser 1440 × 900
- A made-up marketplace in the run's own scratch folder, three plugins, its `featured` list in an order that isn't by name (Zeta, then Acme), Mid not featured:
  - `M="$(dirname "$ECHOCTL_HOME")/market-featured"; for p in zeta-notes acme-notes mid-notes; do mkdir -p "$M/plugins/$p"; done`
  - `echo '{"name":"order-test","featured":["zeta-notes","acme-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes"},{"id":"mid-notes","source":"./plugins/mid-notes"},{"id":"zeta-notes","source":"./plugins/zeta-notes"}]}' > "$M/echo-marketplace.json"`
  - `for p in zeta:Zeta acme:Acme mid:Mid; do echo "{\"id\":\"${p%%:*}-notes\",\"name\":\"${p#*:} notes\",\"version\":\"0.1.0\",\"summary\":\"A made-up plugin of the ordering test\"}" > "$M/plugins/${p%%:*}-notes/echo-plugin.json"; done`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `target/debug/echoctl plugin marketplace add "$M" --name order-test` | `Added order-test: 3 plugins` |
| 2 | `target/debug/echoctl plugin marketplace list` | JSON, a list of the marketplaces; `order-test`'s entry has `count` 3 and `plugins`: `acme-notes`, `mid-notes`, `zeta-notes` (the catalog's order), each with its `name` ("Acme notes"…), `version` "0.1.0", `featured` (true for acme and zeta, false for mid) and `installed` null (not installed; true from this marketplace, false from another). The built-in marketplace's entry lists its plugins too |
| 3 | `target/debug/echoctl plugin marketplace hide order-test`; `target/debug/echoctl plugin marketplace list` | `order-test` is `hidden` true and still lists its three plugins (hidden from Discover, not from the list) |
| 4 | `target/debug/echoctl plugin marketplace show order-test`; open `{{base_url}}/#settings/plugins`, tab "Discover"; type `ordering` in "Search plugins" (only order-test's plugins have it) | Search on: no "Featured" heading, the three cards in one grid — "Zeta notes", "Acme notes" (featured, in the marketplace's order), then "Mid notes" |
| 5 | Clear the search box (chip "All") | Under "Featured": the other marketplaces' featured first (the built-in one's, then each added one's as Marketplaces lists them — on standard data Meeting timer, then Standup Notes), then "Zeta notes" right before "Acme notes" (order-test's `featured` order, not by name); "Mid notes" under "All plugins" |
| 6 | `echo '{"name":"order-test","featured":["acme-notes","zeta-notes"],"plugins":[{"id":"acme-notes","source":"./plugins/acme-notes"},{"id":"mid-notes","source":"./plugins/mid-notes"},{"id":"zeta-notes","source":"./plugins/zeta-notes"}]}' > "$M/echo-marketplace.json"`; `target/debug/echoctl plugin marketplace update order-test`; load `{{base_url}}/#settings/plugins` again (a page reload), tab "Discover" | Under "Featured", after the other marketplaces' ones: "Acme notes" right before "Zeta notes" |

## Teardown
- `target/debug/echoctl plugin marketplace remove order-test`; `tests/fixtures/serve.sh down <dir>`; close the browser tab

## Notes
- `marketplace show <name>` is the opposite of `hide` (its plugins back in Discover), not a listing: `list` lists each marketplace's plugins (#589).
- The featured order across marketplaces: each marketplace's own order, the marketplaces as Discover lists them (the built-in one first).
