---
id: TC-102
title: A plugin reads the meetings Echo last read from the calendar, by start and by days with their threads and client, the user's name and email, and the people of the threads
priority: medium
type: functional
requirements: [F-PLUG-71, F-PLUG-107]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, echo-read, calendar]
---

# TC-102: A plugin reads the meetings Echo read and who the user is

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake`; CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- Data demo installed: in S-36, stage 1. On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes`

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call data-demo calendar` | Today's meetings, each `{id, subject, start, end, organizer, attendees, join_url, response, threads}`, in the order of their `start`; "Acme weekly sync" with `"threads": ["acme-rollout"]` and its attendees by name and email; "Globex steering" with `"threads": ["globex-pilot"]` |
| 2 | Read `$ECHOCTL_HOME/.echo/agenda.json` (Echo's agenda) | The same meetings, by the same ids: the plugin got what Echo had read, nothing more |
| 1a | The same answer | Each meeting also has `client`: "Acme weekly sync" `{"id": "acme", "name": "Acme"}`, "Globex steering" `{"id": "globex", "name": "Globex"}`, a meeting in no thread `null` |
| 1b | `echoctl plugin call data-demo calendar '{"from":"<today>","to":"<today>"}'`; `echoctl plugin call data-demo calendar '{"from":"<tomorrow>"}'` | The same meetings as step 1; `[]` (none starts after today) |
| 3 | `echoctl plugin call data-demo me` | `{"name": "Sam Rivera", "email": "sam@initech.example"}` |
| 4 | `echoctl plugin call data-demo people` | The people of the open threads, each `{key, name, aliases, email, also, organisation, client, title, threads, places, reports_to, reports, link}`; Kim Park: `"key": "kim@globex.example"`, `"email": "kim@globex.example"`, `"organisation": "Globex"`, `"client": "globex"`, `"threads": ["globex-pilot"]`, `"aliases": []`, `"places": [{"thread": "globex-pilot", "role": "sponsor", "stance": "supporter"}]`, `"reports_to": null`, `"reports": []` (no facts or quotes in any row) — her `key` is what a call's `participants[].person` names (TC-100, TC-101) |

## Expected Final State
Nothing changed.

## Teardown
- In S-36: none. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- No request goes to Microsoft 365: the test data has no sign-in, and `calendar` reads only the agenda file Echo keeps (`watch::agenda`).
