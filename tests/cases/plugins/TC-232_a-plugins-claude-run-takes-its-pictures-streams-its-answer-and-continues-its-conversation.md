---
id: TC-232
title: A plugin's Claude run with no tools takes the pictures it holds, streams its answer piece by piece, continues its conversation on its own profile until forgotten with its transcript, and refuses what isn't a picture, one too big or too many
priority: high
type: functional
requirements: [F-PLUG-139]
module: plugins
run: agent
platform: any
data: small
tags: [plugins, host-half, claude-run, fake-claude]
---

# TC-232: A plugin's Claude run takes its pictures, streams its answer and continues its conversation

**Module:** Plugins | **Priority:** High | **Type:** Functional

## Preconditions
- Small data set built right before the run, nothing served: `tests/fixtures/make_data.py <dir> --ai fake`; every Bash call starts with `set -a; . <dir>/env; set +a; D="$(dirname "$ECHOCTL_HOME")"` and aborts unless `$ECHOCTL_HOME` and `$ECHOCTL_CONFIG_DIR` are inside `<dir>`; `echoctl` is `target/debug/echoctl`, built right before the run
- The test data's fake Claude Code: a run whose prompt holds "echo-test: talk" (after `-p`, or in its stream-json message on stdin) answers "Hello from the fake" in two streamed pieces, in the session it was asked to `--resume`, else session `11111111-2222-4333-8444-555555555555`; its arguments one a line in `$D/fake-talk.args`, its stdin message in `$D/fake-talk.stdin`, its working folder in `$D/fake-talk.cwd` (`make_data.py`, `FAKE_CLAUDE`); Run demo's `talk` asks for such a run with `tools: 'none'`, its handles as `images`, `resume` and `onText`, answering `{answer, pieces}`
- Run demo installed: `echoctl plugin marketplace add "$PWD/tests/fixtures/plugins" --name test-plugins`; `echoctl plugin install run-demo --yes`
- Made-up files in `$D/in`: `red.png` (an 8×8 PNG), `board.jpg` (JPEG bytes), `notes.txt` (text), `big.png` (PNG bytes, 3.9 MB), `huge.png` (PNG bytes, over 5 MB)

**Changes:** `<config dir>/plugins/` and `ai.json` (the marketplace, the plugin, a second AI profile); the plugin's store (`$ECHOCTL_HOME/.echo/plugins/run-demo/runs/talk/`); its conversation's record under Echo's state (`plugins/.runs/run-demo/c-lesson-1.json`)

## Test Data

| Field | Value |
|---|---|
| Conversation | `lesson-1` |
| Pictures | `red.png`, `board.jpg` |
| Second profile | `spare`, a local model server that never answers (`http://127.0.0.1:9`) |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `echoctl plugin call run-demo talk '{"resume":"lesson-1"}' --file "$D/in/red.png" --file "$D/in/board.jpg"` | `"answer": "Hello from the fake"`; `pieces`, joined, read `Hello from the fake` (one or more pieces, in order: Echo gathers what comes within 50 ms): the answer whole and as it came |
| 2 | `grep -c -- --input-format "$D/fake-talk.args"; grep -A1 -- '^--tools$' "$D/fake-talk.args"`; the content's kinds of the message in `$D/fake-talk.stdin` (`python3 -c "import json,sys; m=json.loads(open(sys.argv[1]).readline()); print([c.get('source',{}).get('media_type', c['type']) for c in m['message']['content']])" "$D/fake-talk.stdin"`); `sed -n 2p "$ECHOCTL_HOME/.echo/plugins/run-demo/runs/talk/log.txt"`; `cat "$D/fake-talk.cwd"` | `1` (`--input-format stream-json`); `--tools` followed by an empty line (no tools); `['image/png', 'image/jpeg', 'text']`: the pictures in their order, then the prompt; the log's second line ends "with 2 picture(s)"; the folder is `$ECHOCTL_HOME/.echo/plugins/.runs/run-demo/talk` (the plugin's own, not the data folder: its transcripts stay out of the folder session's conversations) |
| 3 | `echoctl plugin call run-demo talk '{"resume":"lesson-1","say":"And the second one?"}'`; `grep -A1 -- '^--resume$' "$D/fake-talk.args"` | The same answer; `--resume` followed by `11111111-2222-4333-8444-555555555555`: the conversation's session continued |
| 4 | `echoctl ai add-local spare --model acme-small --url http://127.0.0.1:9 --name "Spare (test)"`; `echoctl plugin call run-demo talk '{"resume":"lesson-1","profile":"spare"}'` | `added Spare (test) (spare)`; `{"error": "this conversation is on the AI profile “system”, not “spare”: run it on “system”, or forget it (ctx.claude.forget) to begin a new one"}`, at once |
| 5 | Its transcript as Claude Code keeps it: `T="$CLAUDE_CONFIG_DIR/projects/-talk"; S=11111111-2222-4333-8444-555555555555; mkdir -p "$T/$S"; echo '{}' > "$T/$S.jsonl"`; `echoctl plugin call run-demo forget '{"resume":"lesson-1"}'`; `ls "$T"`; forget again; `echoctl plugin call run-demo talk '{"resume":"lesson-1"}'`; `grep -c -- '^--resume$' "$D/fake-talk.args"` | `true`; `ls` lists nothing: the transcript and its folder are gone; `false`; the answer; `0`: a new conversation began |
| 6 | `echoctl plugin call run-demo talk '{}' --file "$D/in/notes.txt"`; the same with `--file "$D/in/huge.png"`; the same with `--file "$D/in/red.png"` given 11 times; the same with `--file "$D/in/big.png"` given 4 times | `{"error": "“notes.txt” isn't a PNG, JPEG, GIF or WebP picture"}`; `{"error": "“huge.png” is over 3.75 MB: a run takes pictures up to 3.75 MB (5 MB base64-encoded, what every platform takes)"}`; `{"error": "a run takes 10 pictures at most, not 11"}`; `{"error": "a run's pictures come to 19.8 MB base64-encoded: 15.0 MB at most in all"}` — each at once, before Claude Code starts |

## Expected Final State
No Claude Code process of the case left; the conversation's record holds the session of the last run on profile `system`.

## Teardown
- `echoctl plugin remove run-demo --delete-data`; `echoctl plugin marketplace remove test-plugins`; `echoctl ai remove spare`; remove `<dir>`

## Notes
- A conversation whose session Claude Code no longer has (forgotten, the run failing so), one marked gone by the plugin's removal mid-run, a conversation's pictures counted across its turns (20, 15 MB), `resume` refused with `copy`, a stream number Echo didn't give refused: Rust, `claudejob::a_conversation_is_forgotten_with_its_transcript_when_asked_when_its_session_is_gone_and_with_its_plugins_data`, `claudejob::a_runs_pictures_and_a_conversations_stay_within_what_every_platform_takes_in_one_request`.
- Another plugin's handle refused, another plugin's conversation of the same name its own, one run at a time in a conversation, a subagent's text not streamed, a session id that isn't one never passed to `--resume`, GIF and WebP: Rust, `jshost::files::a_plugins_run_gets_the_pictures_it_holds_streams_its_answer_and_continues_its_conversation`, `claudejob::a_conversation_continues_its_session_on_its_own_profile_one_run_at_a_time_until_forgotten`, `claudejob::a_streamed_run_hands_over_claudes_own_text_as_it_comes_and_answers_the_same`.
- Verified on Claude Code 2.1.328 by hand (not in a test): image content blocks in a stream-json message, `stream_event` text deltas with `--include-partial-messages`, and `--resume <session>` continuing the same session id in `-p`.
