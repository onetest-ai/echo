---
id: TC-037
title: Two calls started in the same second get two stamps, each keeps its real start and is found by its own stamp, a retitle too
priority: high
type: functional
requirements: [F-REC-22]
module: recording
run: agent
platform: macos
tags: [recording, stamps, file-fed]
---

# TC-037: Two calls started in the same second get two stamps, each found by its own

**Module:** Recording | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make_data.py <dir> --ai fake` (the small set); `target/debug/echoctl` is the debug build from this checkout (`cargo build`)
- Both tracks are fed from files (app_profile.md → "Recording with file-fed calls"): no device is opened, no permission needed; no app is running (`echoctl capture` records by itself)
- Every Bash call starts with the prelude (a fresh shell each time): `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; C=tests/fixtures/call.py; export D`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Two captures started at one instant: `$C start acme-sync --hold; target/debug/echoctl capture --seconds 3 --title "Acme sync" > "$D/a.out" 2>"$D/a.err" & target/debug/echoctl capture --seconds 3 --title "Globex pilot" > "$D/b.out" 2>"$D/b.err"; wait; $C end; tail -n 1 "$D/a.out" "$D/b.out"` | Each prints its call folder: `…/calls/<YYYY-MM-DD_HHMMSS>-acme-sync` and `…/calls/<YYYY-MM-DD_HHMMSS>-globex-pilot`; the two stamps (the first 17 characters of the folder names) differ, one second apart (or two) — though their titles differ, so their names would not have clashed |
| 2 | `for f in a b; do grep -o '"started_at": *"[^"]*"' "$(tail -n 1 "$D/$f.out")/meta.json"; done` | Both `started_at` are in the same second: the earlier stamp's (the real start), not the moved folder's |
| 3 | `A=$(basename "$(tail -n 1 "$D/a.out")"); B=$(basename "$(tail -n 1 "$D/b.out")"); target/debug/echoctl title "$B" "Globex pilot check-in"; ls "$ECHOCTL_HOME/calls" \| grep -e "${A:0:17}" -e "${B:0:17}"` | `title` prints the renamed folder `<B's stamp>-globex-pilot-check-in`; the listing has `<A's stamp>-acme-sync` and `<B's stamp>-globex-pilot-check-in`: the stamp kept |
| 4 | `target/debug/echoctl read "echo://call/${A:0:17}" \| head -1; target/debug/echoctl read "echo://call/${B:0:17}" \| head -1` | `# Call: Acme sync — …` and `# Call: Globex pilot check-in — …`: each stamp finds its own call, the retitled one too |

## Expected Final State
Two calls started in one second are two folders with two stamps; each `meta.json` keeps the real start, and each call is found by its stamp after a retitle.

## Teardown
- Nothing to undo: the run's own data folder (the call file is already hung up in step 1)
