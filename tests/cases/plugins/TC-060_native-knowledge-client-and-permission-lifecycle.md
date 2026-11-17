---
id: TC-060
title: Knowledge's installed client preserves native permissions, state and lifecycle
priority: critical
type: functional
requirements: [F-KNOW-07, F-KNOW-08, F-PLUG-04]
module: plugins
run: agent
platform: any
data: synthetic Knowledge fixture from the echo-plugins checkout
tags: [plugins, knowledge, lifecycle]
---

## Preconditions

Build this Echo checkout. In the echo-plugins checkout run `ECHO_REPO=<this Echo checkout> bash tests/fixtures/knowledge/up.sh <new test directory> <port>`; load its root page first. Every CLI step sources that test directory's `env`. Never use the user's data, vaults or sign-ins.

| Action | Expected Result |
|---|---|
| Inspect the install sheet and open `#kn/acme-knowledge/home`. | The sheet names native vault edits/imports/guarded runs; all seven tabs load from `/plugin/knowledge/`. |
| Read/edit/search a page, inspect a graph, send to fake Claude and prepare/review synthetic ingest changes. | Existing native behavior and profile/approval boundaries hold; no page script errors. The plugin cases TC-001 and TC-002 provide the detailed journey. |
| Use the installed agent command to read a vault page and try direct/encoded mail, calls, preview, jobs and resume views. | Vault reads work; UI-only routes are refused before dispatch. Native human UI pickers remain usable. |
| Configure a synthetic local profile with a model alias, then inspect fake Ask, Ingest/Health and vault-terminal arguments. | The selected provider model name is used; wrapper Ask arguments contain no user message or vault name, and wrapper terminals carry Echo's session proof. |
| Inspect the thread/client choices and hand a synthetic prompt to a vault terminal. | Only label fields reach the client; vault-keyed input reaches its current live terminal and raw terminal IDs are refused. |
| Make a draft, keep a fake conversation live, then disable Knowledge through the test CLI. | The view offers installing/enabling Knowledge, its stylesheet/listeners are removed, and native vault jobs/terminals stop in the running process. |
| Request `/api/kn-plugin/knowledge/read?path=api%2Fkn` while disabled. | The service returns an error saying the plugin is not enabled. A plugin without `uses.knowledge` is also refused. |
| Enable Knowledge without restarting the window, then enter Edit on the same page. | One client/style returns and the draft is restored. Existing registry and receipts remain in place. |
| Remove and reinstall Knowledge in the test environment. | User-owned vault files and native Knowledge state survive. Package removal does not remove the legacy Obsidian plugin's managed notes. |

Native signed-in accounts and Windows desktop checks are person gates, separate from this synthetic hosted-window case.
