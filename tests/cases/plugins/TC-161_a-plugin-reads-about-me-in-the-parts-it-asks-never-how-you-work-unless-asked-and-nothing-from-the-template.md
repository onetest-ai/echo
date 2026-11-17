---
id: TC-161
title: A plugin reads About me in the parts it asks — never how the user works unless asked, a hash that follows an edit — and nothing when About me is missing, empty or the template
priority: medium
type: functional
requirements: [F-PLUG-100]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, about-me]
---

# TC-161: A plugin reads About me in the parts it asks

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data built right before the run, served (the case uses only the CLI): `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; every `echoctl` command is `$E …` (`E=target/debug/echoctl`, built right before) after `. <dir>/env`; abort if `$ECHOCTL_HOME` isn't inside `<dir>`
- Me demo 0.2.0 installed (it asks for `me`): in S-41, stage 1. On its own: `C=$(mktemp -d)/m`; `cp -R tests/fixtures/plugins "$C"`; `cp -R "$C/next/me-demo/." "$C/plugins/me-demo/"`; `$E plugin marketplace add "$C" --name test-plugins`; `$E plugin install me-demo --from test-plugins --yes`

**Changes:** `$ECHOCTL_HOME/.echo/me.md` (About me): rewritten, then removed, then the template

## Test Data

| Item | Value |
|---|---|
| About me of the standard data | "Sam Rivera, delivery lead at Initech. I run the client rollouts for **Acme** and **Globex**." and two lines "- Minutes: …", "- Follow-ups: …" — no headings, no `Name:` line |
| Echo's name for the user | Sam Rivera (`config.json` `me`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `$E plugin call me-demo me` | `text`: About me without its "# About me" heading (from "Sam Rivera, delivery lead at Initech." to "plain mail, no attachments"); `parts`: only `identity`, the same text; `name`: "Sam Rivera" (no `Name:` line: Echo's); `hash`: 8 hex characters |
| 2 | `python3 -c "import sys;open(sys.argv[1],'w').write('# About me\n\nName: Sam R.\nRole: Delivery lead, Initech\n\n## People I work with often\n- Kim Park — Globex sponsor\n\n## How I work — expected patterns\n- I delegate and track.\n\n## How I like notes written\n- Short, in English.\n')" "$ECHOCTL_HOME/.echo/me.md"`; `$E plugin call me-demo me` | `name` "Sam R."; `parts` has `identity` ("Name: Sam R.\nRole: Delivery lead, Initech"), `people` (Kim Park) and `style` ("Short, in English.") and no `patterns`; "I delegate and track." appears nowhere; `hash` another than step 1's |
| 3 | `$E plugin call me-demo me '{"kinds":["style"]}'` | `text` "## How I like notes written\n- Short, in English."; `parts` only `style`; `name` null |
| 4 | `$E plugin call me-demo me '{"kinds":["patterns"]}'` | `parts.patterns` and `text`: "## How I work — expected patterns\n- I delegate and track." — asked, it is given |
| 5 | `$E plugin call me-demo me '{"kinds":["secrets"]}'` | Exit 1: `Error: About me has no part “secrets” (identity, people, terms, style, patterns)` |
| 6 | `rm "$ECHOCTL_HOME/.echo/me.md"`; `$E plugin call me-demo me` | `null` |
| 7 | `$E me`; `$E plugin call me-demo me` | `$E me` prints the path of `.echo/me.md` (made from the template); the call: `null` — the template isn't About me |
| 8 | `python3 -c "import sys;open(sys.argv[1],'w').write('  \n')" "$ECHOCTL_HOME/.echo/me.md"`; `$E plugin call me-demo me '{"kinds":["style"]}'` | `null` |

## Expected Final State
About me is blank; Me demo 0.2.0 installed.

## Teardown
- `$E plugin remove me-demo --delete-data`; `$E plugin marketplace remove test-plugins`; `rm -rf "$C"`; `tests/fixtures/serve.sh down <dir>`

## Notes
- Besides `echoctl`, this case runs `python3 -c` (writing About me as Settings → About me would) and `rm`.
- Name and email alone stay `ctx.echo.read('me')` (TC-102).
- Rust: `jshost::a_plugin_reads_about_me_only_with_uses_me_in_the_parts_it_asks_and_never_the_template`.
