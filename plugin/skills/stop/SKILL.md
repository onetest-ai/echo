---
name: stop
description: Stop the current Echo recording (or cancel a pending arm). Use when the user says "stop recording", "stop", or "don't record this".
---

# echo: stop recording

Run `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl stop` and report its one-line
output. If a recording was stopped, the app transcribes it and the Echo mod runs
`process-call` once the transcript is ready.
