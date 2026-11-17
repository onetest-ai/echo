---
id: TC-104
title: Signed in to Microsoft 365, a plugin searches mail and chats, reads a mail's thread with its attachments' names and a chat's messages over some days
priority: medium
type: functional
requirements: [F-PLUG-72]
module: plugins
run: person
platform: any
data: standard
tags: [plugins, host-half, m365]
---

# TC-104: A plugin reads a mail thread with its attachments and a chat's days

**Module:** Plugins | **Priority:** Medium | **Type:** Functional

## Preconditions
- A person's own Echo signed in to Microsoft 365 (Settings → Calendar & chat → "Sign in…"), a mail thread with an attachment and a Teams chat with messages this week; `echoctl` is that Echo's
- Data demo installed from `tests/fixtures/plugins`: `echoctl plugin marketplace add "<checkout>/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install data-demo --yes`

**Changes:** none in Microsoft 365 (reads only)

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call data-demo search '{"q":"<a word of that thread>"}'` | `results` holds mail hits (`kind: "mail"`, an `id`, `subject`, `from`, `at`) and chat hits (`kind: "chats"`, a `chat` id) of the last 90 days |
| 2 | `echoctl plugin call data-demo thread '{"id":"<a mail hit's id>"}'` | `{subject, picked, messages}`: the thread oldest first, the message with the attachment listing its name under `files` (a signature's picture left out) — no file downloaded |
| 3 | `echoctl plugin call data-demo chat '{"id":"<a chat hit's chat>"}'` | `{id, topic, members, url, since, until, messages}`: the last seven days (`since` six days before today), each message `{id, at, from, text, files}` oldest first |
| 4 | `echoctl plugin call data-demo chat '{"id":"<that chat>","opts":{"since":"<today>","until":"<yesterday>"}}'` | It fails: "the days end before they start" |

## Expected Final State
Nothing sent or changed in Microsoft 365.

## Teardown
- `echoctl plugin remove data-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`

## Notes
- Needs a real Microsoft 365 account (`tests/app_profile.md` → Out of Scope): never with test data.
