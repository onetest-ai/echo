---
id: TC-205
title: Against the fake Microsoft 365, a plugin searches mail, reads a mail's thread with its attachments' names and a chat's messages, and nothing reaches the real one
priority: medium
type: functional
requirements: [F-PLUG-72, F-PLUG-120]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, m365, fake-service]
---

# TC-205: A plugin reads mail and a chat from the fake Microsoft 365

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- Small data set built right before the run, nothing served: `tests/fixtures/make_data.py <dir> --ai fake`; every Bash call starts with `. <dir>/env` and aborts unless `$ECHOCTL_HOME` is inside `<dir>`; `echoctl` is `target/debug/echoctl` (a debug build: only it reads `ECHOCTL_TEST_GRAPH`)
- The fake Microsoft 365 on port 48270, and the env pointing at it (`tests/app_profile.md` → Plugins against fake services):

```bash
nohup python3 tests/fixtures/fake_graph.py 48270 --log <dir>/graph.log >/dev/null 2>&1 & echo $! > <dir>/graph.pid
echo 'export ECHOCTL_TEST_GRAPH=http://127.0.0.1:48270' >> <dir>/env
```

- Data demo installed: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes`

**Changes:** `<config dir>/plugins/` (the marketplace and the plugin); `<dir>/graph.log`

## Test Data

| Field | Value |
|---|---|
| Mail (fake) | "Acme pilot plan" from Jane Doe (`AAMk-acme-pilot-1`, `acme-pilot-plan.pdf` and an inline `signature.png`), "RE: Acme pilot plan" from Sam Rivera (`AAMk-acme-pilot-2`), one conversation `conv-acme-pilot` |
| Chat (fake) | `19:acme-pilot@thread.v2`, "Acme pilot", Jane Doe and Sam Rivera, two messages today |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call data-demo search '{"q":"pilot plan","opts":{"kinds":["mail"]}}'` | `errors` null; `results` two mail hits (`kind: "mail"`): `AAMk-acme-pilot-1` "Acme pilot plan" from Jane Doe and `AAMk-acme-pilot-2` "RE: Acme pilot plan" from Sam Rivera, each with `conversation: "conv-acme-pilot"` |
| 2 | `echoctl plugin call data-demo thread '{"id":"AAMk-acme-pilot-2"}'` | `subject` "RE: Acme pilot plan", `picked` "AAMk-acme-pilot-2"; `messages` oldest first: `AAMk-acme-pilot-1` with `files: ["acme-pilot-plan.pdf"]` (no `signature.png`), then `AAMk-acme-pilot-2` with `files: []` and `text` "Thanks, the pilot starts on Monday." (the quoted mail cut off) |
| 3 | `echoctl plugin call data-demo chat '{"id":"19:acme-pilot@thread.v2"}'` | `topic` "Acme pilot", `members` Jane Doe and Sam Rivera; `messages` oldest first: Jane Doe "Is the pilot plan final?", then Sam Rivera "Yes, the pilot starts on Monday." with `files: ["acme-pilot-plan.pdf"]`; `until` today, `since` six days before |
| 4 | `echoctl plugin call data-demo thread '{"id":"AAMk-none"}'` | It fails: "Graph: ErrorItemNotFound Not in the fake Microsoft 365" |
| 5 | `echoctl calendar login` | It fails: "ECHOCTL_TEST_GRAPH is set: a test never signs in to the real Microsoft 365" — no browser, nothing in `<dir>/open.log` |
| 6 | `ECHOCTL_TEST_GRAPH=https://graph.microsoft.com echoctl plugin call data-demo thread '{"id":"AAMk-acme-pilot-2"}'` | It fails: "Microsoft 365 isn't connected: sign in to it in Echo's Settings → Calendar & chat" (not a local address: no sign-in) |
| 7 | `cat <dir>/graph.log` | Only the requests of steps 1–4: one `POST /search/query`, then `GET /me/messages/…`, `/me/messages?$filter=conversationId…`, `/me/messages/AAMk-acme-pilot-1/attachments…`, `/chats/19%3Aacme-pilot%40thread.v2…`; nothing for steps 5 and 6 |

## Expected Final State
Nothing sent; the fake was the only Microsoft 365 reached.

## Teardown
- `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `kill "$(cat <dir>/graph.pid)"`; remove `<dir>`

## Notes
- The fake (`tests/fixtures/fake_graph.py`) serves only what `ctx.m365` asks for; the same reads on a real account: TC-104 (`run: person`). Rust: `jshost::a_plugin_reads_a_mail_thread_from_the_fake_microsoft_365_set_for_tests`, `graph::the_fake_graph_is_only_a_local_address`.
