---
id: S-35
title: Knowledge as an installed plugin with native guards and preserved vaults
run: agent
data: synthetic Knowledge fixture in echo-plugins
cases:
  - tests/cases/plugins/TC-054_native-knowledge-client-and-permission-lifecycle.md
---

# S-35: Knowledge as an installed plugin

Sam installs Knowledge, reads/edits a connected vault, asks with its pinned profile, reviews ingest/health changes,
then disables and re-enables the plugin while preserving an unsaved draft. The native engine enforces the same
paths, profiles and permission cards throughout.

Use TC-054's fixture and execute the Knowledge marketplace cases TC-001, TC-002 and TC-003 in order. Retain one
browser tab for the journey, report each subcheck separately, and stop the fixture's server afterward. Native
window inspection, real-account integrations and native Windows remain separately recorded checks.
