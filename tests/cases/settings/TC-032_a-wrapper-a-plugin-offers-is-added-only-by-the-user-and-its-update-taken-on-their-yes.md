---
id: TC-032
title: A wrapper a plugin offers is added only by the user, its update is taken only on their yes, and the profile stays when the plugin goes
priority: high
type: functional
requirements: [F-AI-14, F-AI-15]
module: settings
run: agent
platform: macos
data: standard
tags: [ai, profiles, wrapper, plugins]
---

# TC-032: A wrapper a plugin offers is added only by the user; its update is taken on their yes

**Module:** Settings | **Priority:** High | **Type:** Functional

## Preconditions
- Every `echoctl` command is `$E …` (`E=target/debug/echoctl`) after `. <dir>/env`; `D="$(dirname "$ECHOCTL_HOME")"`
- **Two made-up company wrappers on the PATH the window's shell sees**, in a folder outside the run's data: `W=$(mktemp -d)/wrappers`; `$W/acme-claude` and `$W/acme-claude2`, each the script of claude/TC-031's preconditions (`chmod +x`). The window is served with them on its PATH: `PATH="$W:$PATH" tests/fixtures/serve.sh up <dir> <port> --set standard`; CLI steps run with `export PATH="$W:$PATH"` too
- A copy of the test marketplace (its wrapper-demo gets updated): `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `$E plugin marketplace add "$C" --name test-plugins`
- `{{base_url}}/` was loaded just before the first window step

**Changes:** `<config dir>/ai.json`, the installed plugins, `$C/plugins/wrapper-demo/echo-plugin.json`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin install wrapper-demo --from test-plugins` | The sheet: "Offers an AI profile that starts acme-claude — a program you install yourself — in place of Claude Code; it sees and can change everything Claude in Echo is sent, call transcripts included. You add it in Settings → AI & minutes, if you want it"; `Error: run again with --yes to agree`. Then the same with `--yes`: `Installed Wrapper demo 0.1.0` |
| 2 | `$E ai status` | `wrapper_offers`: one, `"plugin": "wrapper-demo"`, `"name": "Acme Claude"`, `"command": "acme-claude"`, `"profile": null`; no profile of kind `wrapper`, and `"default"` still `work` (installing added nothing) |
| 3 | Navigate to `{{base_url}}/#settings/ai`; read the card "Profiles" | A row "Acme Claude" tagged "Offered by Wrapper demo": "Starts acme-claude — a program you install yourself — in place of Claude Code. It sees and can change everything Echo sends to Claude, call transcripts included.", with "Add…" |
| 4 | Click "Add…" on it; wait 3 s | A row "Add the wrapper Wrapper demo offers" with `acme-claude`, "Check", "Add profile" (enabled) and "Cancel"; under them "Found <$W>/acme-claude — 2.1.400 (Claude Code)" |
| 5 | Click "Add profile"; wait 3 s | A notice "Added Acme Claude — it starts <$W>/acme-claude"; the offer row is gone; a profile row "Acme Claude" tagged "Wrapper": "Starts <$W>/acme-claude in place of Claude Code · offered by Wrapper demo. It sees and can change everything Echo sends to Claude, call transcripts included.", with "Make default" — not the default |
| 6 | `cp "$C/next/wrapper-demo/echo-plugin.json" "$C/plugins/wrapper-demo/echo-plugin.json"`; `$E plugin marketplace update test-plugins`; `$E plugin update wrapper-demo` | Asks again: the sheet's line now names acme-claude2, marked `(new)`, and `Error: run again with --yes to agree`. Then with `--yes`: `Updated Wrapper demo to 0.2.0` |
| 7 | `$E ai status` | The `wrapper-demo` profile's `"command"` still ends `/acme-claude`; its offer has `"profile": "wrapper-demo"` and `"update"` that same path |
| 8 | Reload `{{base_url}}/#settings/ai`; read the "Acme Claude" row | Under its models: "Wrapper demo's update offers acme-claude2 instead — this profile keeps starting its program until you take it.", and a button "Use it" |
| 9 | Click "Use it"; wait 3 s | The row says "Starts <$W>/acme-claude2 in place of Claude Code"; no "Use it" |
| 10 | `$E plugin disable wrapper-demo`; `$E ai status`; reload `{{base_url}}/#settings/ai` | Switched off: `wrapper_offers` is `[]`, the profile `wrapper-demo` is still there and its row the same, with no offer row. Then `$E plugin enable wrapper-demo` |
| 11 | `$E plugin remove wrapper-demo`; `$E ai status` | The profile `wrapper-demo` (kind `wrapper`, `"offered_by": "wrapper-demo"`) is still there; `wrapper_offers` is `[]` |

## Expected Final State
One wrapper profile the user added, starting `$D/wrappers/acme-claude2`; no plugin offers one.

## Teardown
- `$E ai remove wrapper-demo`; `$E plugin marketplace remove test-plugins`.

## Notes
- The CLI does the same: `$E ai add-wrapper --plugin wrapper-demo` and `$E ai update-wrapper wrapper-demo`.
