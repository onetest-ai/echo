---
id: S-35
title: Knowledge as an installed plugin with native guards and preserved vaults
run: agent
data: synthetic Knowledge fixture in echo-plugins
cases:
  - tests/cases/plugins/TC-060_native-knowledge-client-and-permission-lifecycle.md
---

# S-35: Knowledge as an installed plugin

Sam installs Knowledge, reads/edits a connected vault, asks with its pinned profile, reviews ingest/health changes,
then disables and re-enables the plugin while preserving an unsaved draft. The native engine enforces the same
paths, profiles and permission cards throughout.

Use TC-060's fixture and execute the Knowledge marketplace cases TC-001, TC-002 and TC-003 in order. Retain one
browser tab for the journey, report each subcheck separately, and stop the fixture's server afterward. Native
window inspection, real-account integrations and native Windows remain separately recorded checks.

## Assets

| Asset | Made by | Used by stages |
|---|---|---|
| Synthetic Echo data, connected Acme vault, installed Knowledge and fake AI profile | Knowledge fixture, once in the prelude | TC-060 / marketplace TC-001..003 |
| Edited page, draft, fake conversation and ingest scratch copy | Marketplace TC-001 / TC-002 | TC-002 / TC-003 |
