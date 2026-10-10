---
name: note
description: Add the user's own note to the call being recorded right now — a point to remember, a decision, a heading. These notes become the backbone of the call's summary. Use when the user says "note …", "remember that …", or types notes during a call.
argument-hint: "<text>  (start with # for a heading)"
---

# echo: note during a call

Run `${CLAUDE_PLUGIN_ROOT}/scripts/echoctl note "<text>"` with the user's words as given (keep
their wording and language; don't rewrite). A line starting with `#` becomes a heading in the
final notes. Confirm in a couple of words ("Noted."). If nothing is recording, say so and offer
`/echo:record`.
