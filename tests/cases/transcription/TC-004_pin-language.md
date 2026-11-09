---
id: TC-004
title: A language pinned for one call applies to that call only
priority: medium
type: functional
requirements: [F-STT-02]
module: transcription
run: agent
platform: macos
tags: [transcription, languages, recordings]
external_id: TC-TRN-005
---

# TC-004: A language pinned for one call applies to that call only

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- The recordings of this build are prepared (`tests/fixtures/recordings.py prepare`: `app_profile.md` → "Recorded calls"). With the workspace languages `en,uk,ru` it captured `mixed-languages` (Ukrainian, Russian and English lines, 31 s), fed from files, with `echoctl capture --languages uk` (`mixed-languages-pinned`), printed the config after it (`config.after.json`), and transcribed it; then it transcribed the same tracks once more as a call with no pin of its own (`mixed-languages`, the "next call")
- Every command after `. <dir>/env` on test data built by `tests/fixtures/make-data.sh <dir>`; `P="$(python3 tests/fixtures/recordings.py path)"`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `grep -A3 '"language' "$P/mixed-languages-pinned/meta.json"` | The capture wrote the pin into the call's `meta.json`: `"language": "uk"` (and `languages` ["uk"]) |
| 2 | `CALL="$(python3 tests/fixtures/recordings.py copy mixed-languages-pinned --title "TRN-004 pinned")"; grep -o '"lang": *"[a-z]*"' "$CALL/segments.json" | sort | uniq -c` | One language value only: `uk`, on every segment (no `en`, no `ru`) |
| 3 | `grep -A5 '"languages"' "$P/mixed-languages-pinned/config.after.json"` | The workspace `languages` are still ["en", "uk", "ru"] after the pinned call: the pin did not change the setting |
| 4 | `grep "  language:" "$P/mixed-languages/transcribe.out"` | "language: uk" with no "(from … utterances)": the next call, with no pin of its own, is in the default language (the workspace's first, `uk,ru,en`), not detected |
| 5 | `CALL="$(python3 tests/fixtures/recordings.py copy mixed-languages --title "TRN-004 next")"; grep -o '"lang": *"[a-z]*"' "$CALL/segments.json" | sort | uniq -c` | One language value on every segment, `uk` (a call has one language) |

## Expected Final State
A language pinned for one call applies to that call only; the workspace languages and the next call are untouched.

## Notes
- Since 2026-11-09 nothing detects a call's language: a call without a pin is in the default language, the workspace's first. The prepared next call's workspace list is `uk,ru,en`, so its default is `uk`.
- The pin is `capture --languages`; the record menu's "Only Ukrainian" and `echoctl arm --languages uk` write the same `languages` into `meta.json`.
- Before 2026-11-08 the case captured two calls and transcribed both itself (and the next call was `very-short`, English only, which showed less); now the capture and both transcriptions are made once per build.
- Run 2026-11-08 evening (S-17, file-fed `prepare`): PASS — 8 segments all `uk`; workspace languages untouched; the next call 6 `uk`, 1 `ru`, 1 `en`.
- Run on 2026-11-07 (S-17): PASS but for `capture`'s exit code (#141): 8 segments all `uk`; the next call `en`; workspace languages untouched.
