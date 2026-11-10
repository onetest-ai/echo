---
id: TC-013
title: Quitting Echo right after it transcribed for another computer ends cleanly, with no crash report
priority: medium
type: functional
requirements: [F-STT-10]
module: transcription
run: agent
platform: macos
tags: [transcription, server, quit]
---

# TC-013: Quitting while transcribing for other computers ends cleanly

**Module:** Transcription | **Priority:** Medium | **Type:** Functional

## Preconditions
- Test data from `tests/fixtures/make_data.py <dir> --ai fake`, the debug build, a local speech model linked and active (`recording/TC-001` step 1). The prelude: `. <dir>/env; D="$(dirname "$ECHOCTL_HOME")"; W=tests/fixtures/wait.py; S="$ECHOCTL_CONFIG_DIR/transcription-serve.json"; export D S`
- No app running on the test data

## Test Data

| Field | Value |
|-------|-------|
| Port | 48171 (any free port) |
| Utterance | 1 s of silence, language `en` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run the prelude, then `target/debug/echoctl server share on`, `python3 -c "import json,sys; p=sys.argv[1]; s=json.load(open(p)); s['port']=48171; json.dump(s, open(p,'w'))" "$S"`, `ls ~/Library/Logs/DiagnosticReports \| grep -c '^echoctl' > "$D/crashes.before"`, `nohup target/debug/echoctl app run --no-tray --background > "$D/app.out" 2>&1 & echo $! > "$D/app.pid"`, `$W 30 app`, `sleep 3` | "waited … for app"; the app serves on port 48171 |
| 2 | Run the prelude, then `python3 -c "import json,struct,sys; h=json.dumps({'languages':['en']}).encode(); open(sys.argv[1],'wb').write(struct.pack('<I',len(h))+h+b'\0\0'*16000)" "$D/body.bin"`, `curl -s -o /dev/null -w '%{http_code}\n' -H "Authorization: Bearer $(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['key'])" "$S")" --data-binary @"$D/body.bin" http://127.0.0.1:48171/v1/utterance` | `200`: the server loaded its model and decoded the utterance |
| 3 | Run the prelude, then `target/debug/echoctl app quit; $W 60 no-app; sleep 6; echo $(( $(ls ~/Library/Logs/DiagnosticReports \| grep -c '^echoctl') - $(cat "$D/crashes.before") )); grep -c 'ggml_abort' "$ECHOCTL_HOME/.echo/app.log"` | "waited … for no-app", then `0` and `0`: the quit let go of the server's model, no crash report and no abort in the app's log |

## Expected Final State
The app quit right after serving a request, with its model still loaded, and left no crash report.

## Teardown
- Run the prelude, then `target/debug/echoctl server share off`; if the app is still running, `kill $(cat "$D/app.pid")`

## Notes
- Other processes may write crash reports of their own meanwhile: step 3 counts only `echoctl`'s, and a count above 0 is checked against its report (`ggml_metal_device_free` at exit is this case's failure, #271). The app started with `--background` writes its output to `$ECHOCTL_HOME/.echo/app.log`, not to `app.out`.
- The server listens on all interfaces while sharing is on: run it on a trusted network and turn sharing off at the end.
