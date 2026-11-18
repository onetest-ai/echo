---
id: TC-018
title: After an update Echo removes the index's old snapshots and the page reader's page images once, and keeps Intake's facts, the dictionary, the senders and the sources' state
priority: medium
type: functional
requirements: [F-KNOW-09]
module: lifecycle
run: person
platform: macos
tags: [lifecycle, update, recall, intake]
---

# TC-018: An update removes the old snapshots and page images, and keeps Intake's files

**Module:** Lifecycle | **Priority:** Medium | **Type:** Functional

## Preconditions
- The person's own Echo dev build of this version (`cargo build`), not running, on made-up data
  (`tests/fixtures/make-data.sh <dir>`, `. <dir>/env`), as if updated from a version that kept snapshots and page images:

```bash
R="$ECHOCTL_HOME/recall"
mkdir -p "$R/snapshots" "$R/assets/ab12" "$R/intake/facts/mail" "$R/taxonomy"
echo x > "$R/snapshots/knowledge-20261101-120000000.sqlite"; echo x > "$R/assets/ab12/p01.png"
echo '{}' > "$R/intake/facts/mail/acme1.json"; echo '{}' > "$R/senders.json"; echo '{}' > "$R/sources.json"
```

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Start the app on the data (`target/debug/echoctl watch`); wait 10 s | It starts; its log has `recall: snapshots/ removed (no longer used)` and `recall: assets/ removed (no longer used)` |
| 2 | `ls "$R"` | No `snapshots` and no `assets`; `intake/`, `taxonomy/`, `senders.json` and `sources.json` are there; `intake/facts/mail/acme1.json` is unchanged |
| 3 | Quit the app; start it again; wait 10 s | Neither line is logged again; the folder is as in step 2 |

## Expected Final State
The two folders are gone; nothing else under `recall/` changed.

## Notes
- The same as a Rust test: `brain::tests::the_snapshots_and_page_images_go_and_intakes_files_stay`.
