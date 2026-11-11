---
id: TC-028
title: A mail source can be limited to subjects — the words are kept and shown, and mail from anyone needs words
priority: high
type: functional
requirements: [F-THR-21]
module: threads
run: agent
platform: any
tags: [threads, profile, sources, mail]
---

# TC-028: A mail source can be limited to subjects — the words are kept and shown, and mail from anyone needs words

**Module:** Threads | **Priority:** High | **Type:** Functional

## Preconditions
- Test data built by `tests/fixtures/make-data.sh <dir>`; window served at `{{base_url}}` (`tests/fixtures/serve.sh up <dir> <port>`) and `{{base_url}}/` loaded first (the cookie); every `echoctl` command runs as `target/debug/echoctl …` after `. <dir>/env`
- Recall is off and Microsoft 365 is not signed in (the test data's default): the Profile shows a thread's sources as chips without reading them

## Test Data

| Field | Value |
|-------|-------|
| Thread | Acme rollout (`acme-rollout`) |
| Address | `jane@acme.example` |
| Words | `Acme Platform, APX`, later `Rollout` |

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | Run `target/debug/echoctl project source acme-rollout --mail jane@acme.example --subject "Acme Platform, APX"` | It prints the thread's sources: one `{"kind": "mail", "id": "person:jane@acme.example", "subject": ["Acme Platform", "APX"]}` |
| 2 | Run `target/debug/echoctl project source acme-rollout --mail '*'` | It fails (exit code not 0) saying mail from anyone needs `--subject`; `target/debug/echoctl project show acme-rollout` still lists one mail source |
| 3 | Run `target/debug/echoctl project source acme-rollout --mail '*' --subject "Acme Platform"` | The sources now also hold `{"kind": "mail", "id": "any:*", "subject": ["Acme Platform"]}` |
| 4 | Run `target/debug/echoctl project source acme-rollout --mail jane@acme.example --subject Rollout` | Still one source `person:jane@acme.example`, its `"subject"` now `["Rollout"]` (the words replaced, no second source) |
| 5 | Navigate to `{{base_url}}/#project/acme-rollout/profile` and read the Sources row | Two chips: "✉ mail with jane@acme.example · subject: Rollout" and "✉ any mail · subject: Acme Platform" |

## Expected Final State
Acme rollout has two mail sources, each limited to its words.

## Teardown
- `target/debug/echoctl project source acme-rollout --remove mail:person:jane@acme.example` and `… --remove mail:any:*`

## Changes
- Acme rollout's sources (removed again in the teardown).

## Notes
- Which mail such a source reads takes a real mailbox: `threads/TC-029`. The rule itself is `ledger::mail::tests` and `brain::sources::tests`.
- The Sources form's subject field shows only with Recall on and Microsoft 365 signed in: `threads/TC-029` step 1.
