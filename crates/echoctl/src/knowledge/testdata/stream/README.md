# Claude Code stream-json recordings

Real output of Claude Code **2.1.310**, recorded on 6 November 2026 on macOS with a made-up vault (the starter vault
plus one invented client page, *Northwind Retail*, with invented people). Nothing here is a real person, client or page.
Plan 2, Task 0 (spec V10, V11); the protocol is written up in `.ai-run/guides/design/knowledge.md` (*Chat protocol*).

Each file is one run, one JSON object per line, in the order the lines passed:

- A line `{"_sent": {...}}` is what the host wrote to Claude Code's stdin (a user message, a permission answer, an
  interrupt). Every other line is Claude Code's stdout, unchanged except for the scrub below.
- Command (host side): `claude -p --input-format stream-json --output-format stream-json --verbose
  --include-partial-messages --permission-mode default --permission-prompts host --permission-prompt-tool stdio
  --prompt-suggestions --strict-mcp-config --model haiku --setting-sources project --settings <probe settings>
  --tools <...> --allowed-tools <...>` in the vault root. The probe's settings held a `PreToolUse` hook answering
  *ask* for `Edit`, `Write` and `WebFetch`. `host-only-denied.jsonl` was recorded without `--permission-prompt-tool`
  and without the hook (see the design note: this is why the flag is in the command).

| File | Shows |
|---|---|
| `ask-turn.jsonl` | An Ask turn: `system` `init`, `stream_event` deltas (thinking, `input_json_delta` of a `Skill`, `Grep` and `Read`, `text_delta`), the full `assistant` and `user` (`tool_result`) messages, `post_turn_summary`, `result`. |
| `curate-edit.jsonl` | An `Edit` asked through the host (`control_request` / `can_use_tool`, with the hook's `decision_reason`), answered *allow*; the tool result carries the file's old and new text; then `result` and `prompt_suggestion`. |
| `curate-deny.jsonl` | The same `Edit`, answered *deny*: the `is_error` tool result and `permission_denials` in `result`. The file stays unchanged. |
| `webfetch.jsonl` | A `WebFetch` asked through the host; the request carries the `agent_id` of the `web-fetch` sub-agent that made the call; `task_*` system events. |
| `interrupt.jsonl` | An `interrupt` control request sent 2.5 s into a long answer: its `control_response`, the `[Request interrupted by user]` message and the `error_during_execution` `result`. |
| `suggestion.jsonl` | A tool turn ending in a `prompt_suggestion` line after the `result`. |
| `resume.jsonl` | `--resume <id>` of the `ask-turn` session: a follow-up answered from the earlier context, under a new `session_id`. |
| `host-only-denied.jsonl` | `--permission-prompts host` alone: no `control_request`; the `Edit` is refused with a `permission_denied` system event. |

## Scrub

Absolute paths became `/VAULT/...` and `/HOME/...`; every uuid (session, message, request) became
`00000000-0000-4000-8000-0000000000NN` in order of appearance per file; `toolu_`, `msg_` ids became
`toolu_000000000000000000000001`-style counters; thinking signatures became `SIGNATURE`; `rate_limit_event` keeps only its
`status` and `rateLimitType`. The `system` `init` event keeps only `type`, `subtype`, `cwd`, `session_id`, `tools`,
`model`, `permissionMode` and `claude_code_version` (the account, organisation, API-key source and the lists of plugins,
skills, slash commands, agents and MCP servers are dropped). `input_json_delta` fragments were re-split after the
path scrub, so each block still joins to valid JSON.

The model was `haiku`; thinking text arrives empty (only `estimated_tokens`). Costs are fractions of a cent.
