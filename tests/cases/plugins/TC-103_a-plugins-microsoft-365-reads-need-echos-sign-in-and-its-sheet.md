---
id: TC-103
title: A plugin's Microsoft 365 reads are only those its sheet names, never a send, and without Echo's sign-in each says so
priority: high
type: functional
requirements: [F-PLUG-72]
module: plugins
run: agent
platform: any
data: standard
tags: [plugins, host-half, m365]
---

# TC-103: A plugin's Microsoft 365 reads need Echo's sign-in and its sheet

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Standard data set built right before the run: `tests/fixtures/serve.sh up <dir> <port> --set standard --ai fake` (Microsoft 365 isn't connected in it); CLI steps run in Bash after `. <dir>/env`; `echoctl` is `target/debug/echoctl`
- Data demo installed: in S-36, stage 1 (its sheet's Microsoft 365 line is checked there). On its own: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes`

**Changes:** none

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin list` | Under `installed`, data-demo's `uses` holds "Reads your Microsoft 365 mail and Teams chats through Echo's sign-in when it asks: it searches them and reads a mail's thread with its attachments' names or a chat's messages over some days — it never sends" |
| 2 | `echoctl plugin call data-demo search '{"q":"pilot"}'` | It fails: "Microsoft 365 isn't connected: sign in to it in Echo's Settings → Calendar & chat" |
| 3 | `echoctl plugin call data-demo thread '{"id":"AAMkAD-made-up"}'`; `echoctl plugin call data-demo chat '{"id":"19:made-up@thread.v2"}'` | Each fails with the same "Microsoft 365 isn't connected" |
| 4 | `echoctl plugin call data-demo search '{"q":"pilot","opts":{"kinds":["files"]}}'` | It fails: "data-demo didn't ask to read your Microsoft 365 files when it was installed" (asked before anything is tried) |

## Expected Final State
Nothing changed; nothing was sent.

## Teardown
- In S-36: none. On its own: `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `tests/fixtures/serve.sh down <dir>`

## Notes
- The plugin API has no send for Microsoft 365 at all. A plugin with only `mail` is refused `chat` ("didn't ask to read your Microsoft 365 chats"): `jshost::a_plugin_reads_microsoft_365_only_as_its_sheet_said_and_only_on_echos_sign_in`. The reads themselves, on a real account: TC-104 (`run: person`).
