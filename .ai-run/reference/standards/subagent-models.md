# Subagent models — reference

Which model a subagent gets, derived from Echo's task history. The rule is AGENTS.md's Working Rule 5; this reference
holds the tree it points to, the evidence and the current model names. It changes when the history shows a new factor
or a model name changes.

## Current models per class

| Class | Model (Agent tool `model`) | Id as of 2026-11-15 |
|---|---|---|
| Strongest | `opus` | `claude-opus-5-5` |
| Middle | `sonnet` | `claude-sonnet-5-5` |
| Cheapest | `haiku` | `claude-haiku-5-5` |

- An Agent call without `model` inherits the lead's model; a `general-purpose` subagent of an Opus lead is Opus.
- The QA agents that `tests/install.sh` installs (`test-runner`, `test-author`, `test-reporter`, …) carry their own
  model in their definition; a `model` given in the call overrides it.
- `fork` always runs on the lead's model.

## The history it is drawn from

To 2026-11-15: 523 subagents in the project's session transcripts (claudecho and echo-plugins), and 401 merged PRs of
`onetest-ai/echo` with the issues filed against them.

| Who | Model | Count | What they did |
|---|---|---|---|
| general-purpose | Opus | 312 | every implementation, review, re-review, audit, root-cause and spike |
| Explore, fork | Opus | 8 | code surveys; functional runs of a client plugin set |
| test-runner, test-run-lead | Sonnet | 152 | running written cases and scenarios |
| test-author, app-profiler | Sonnet | 14 | writing and rewriting functional cases; profiling the window |
| general-purpose | Sonnet | 3 | case-to-feature links, case wording fixes |
| test-reporter | Haiku | 19 | run reports from results |
| web-fetch, claude-code-guide | Haiku | 13 | docs and page lookups |
| general-purpose | Haiku | 2 | capability probes |

230 of the Opus agents were reviews; 45 of them got at least one re-review round by message.

## Task table

Rounds = review rounds until clean. Findings: real (high/medium/low), speculative, nit; kind in brackets. Escaped = a
bug filed after the merge. ✓ = the tree picks what was used; ✗ = it picks differently (see Counter-checks).

| # | Task | Kind | Areas | Spec | Size | Used | Rounds | Findings | A weaker model would plausibly miss | Tree |
|---|---|---|---|---|---|---|---|---|---|---|
| 1 | Audit plugin rules guides | audit | guides, plugins, security | judgment | read-only | Opus | 1 | real high: `ctx.claude.installPlugin` ungated [security hole]; 3 wrong rules | the hole: found by reading rules against code, no review of the code had seen it | Opus ✓ |
| 2 | #576 installPlugin only the named channel plugin | fix | security, plugins, permissions | clear | +135 −6, 7 files | Opus | 2 | real low: install profile argument [edge case] | — | Opus ✓ |
| 3 | Audit non-plugin rules guides | audit | guides | judgment | read-only | Opus | 1 | real: process rules copied in 4 guides and drifted [design] | — | Opus ✓ |
| 4 | #446 thread sync reads an item for each thread | feature | ledger, strict routing | design | +193 −8, 9 | Opus | 1 | review: not a bug; escaped #448: other profiles' titles reach the model [security] | missed by Opus too: the brief didn't name the routing boundary | Opus ✓ |
| 5 | #591 routines and Yolo by one choice | feature | permissions, guard, runs | design, owner choices | +1261 −347, 48 | Opus | 5 | real: asking runs lacked the read-only marker, plugin commands could write [security]; owner rework [design] | the marker gap | Opus ✓ |
| 6 | #583 plugin context harness (`.plugins/`, `agent/`) | feature | plugins, security, CLI | design | +2853 −365, 46 | Opus, 14 msgs | 5 | real: tampered files re-laid, prefix collisions, install source [security, edge] | tamper paths | Opus ✓ |
| 7 | echo-plugins#10 migrate to `agent/` | migration | plugins | clear format, per-plugin judgment | 8 msgs | Opus | 2 | nits | — | split ✗ |
| 8 | #568 plugin scripts keep the test open guard | fix | test env guard, plugins | clear | +73 −3, 3 | Opus | 1 | real: a script's `echoctl` on test data acts on real data → #569 [data safety] | the env inheritance chain | Opus ✓ |
| 9 | #595 / #593 fix #569 and #550 | fix | guard, test data, clock | clear | +86 −5; +41 −4 | Opus | 2 | real low: before setup, midnight cap [edge, time] | midnight | Opus ✓ |
| 10 | #67 Claude panel knows the CLI | feature | untrusted text, sending | clear | +112 −58, 5 | Opus | 1 | real: mail to model-picked addresses against the rule [security] | — | Opus ✓ |
| 11 | #27 Obsidian phase A | feature | plugins, untrusted text, folders | design | +832 −22, 13 | Opus | 1 | medium: one missing vault stops the plugin; listeners doubled [edge, regression] | — | Opus ✓ |
| 12 | #21 marketplaces, plugins with code | feature | plugins, security | design | +3880 −38, 18 | Opus | 2 parallel (security, correctness) | reviewed by concern, one reviewer each | — | Opus ✓ |
| 13 | #564 export a page as HTML | feature | untrusted text to HTML, Windows paths | clear | +495 −3, 14 | Opus | 1 | real low: Windows Downloads guessed; sanitizer only in a manual case [test gap] | — | Opus ✓ |
| 14 | #406 too-old Claude Code refused | fix | guard, runs | clear | +58 −15, 8 | Opus | 1 | clean | — | Opus ✓ |
| 15 | #405 deliver to the user only | fix | sending | clear | +9 −12, 4 | Opus | 1 | 2 real low, 1 speculative | — | Opus ✓ |
| 16 | Yolo thread reads | fix | guard, runs | clear | small | Opus | 1 | speculative: refusal test dropped [test gap] | — | Opus ✓ |
| 17 | #525 Add a widget keeps the preview in sight | feature | UI state (hover, focus, scroll) | spec + design | +123 −42, 8 | Opus | 4 | each round new real low: wrong widget added, focus lost, list jumps; Windows-only freeze [regression, edge] | every one of them | Opus ✓ |
| 18 | #623 S-24 step 43 late in the evening | root cause + fix | UI scroll, clock | unknown cause | +146 −13, 7 | Opus, 4 msgs | 3 | wrong design twice (guessing from input events, then frame counting) [design] | the approach | Opus ✓ |
| 19 | Today duplicate meeting row while recording | root cause + fix | Today, recording, timing | unknown cause | 272 tool calls | Opus | 1 | — (a test computing now twice) | — | Opus ✓ |
| 20 | #549 redraw widget by widget | feature | UI state | clear | +64 −10, 5 | Opus | 2 | real, fixed in round 1 [regression] | — | Opus ✓ |
| 21 | #539 dashboards learn a thread or client | feature | UI, cross-area | design | +260 −87, 11 | Opus | 3 | real low: card kept on another page, filter [edge] | — | Opus ✓ |
| 22 | #556 dashboards of their own | feature | UI, persistence | design | +578 −27, 16 | Opus | 2 | real low: page id prefix match [edge] | — | Opus ✓ |
| 23 | #552 install reaches the window without restart | feature | concurrency (watcher, reload) | design | +138 −7, 9 | Opus | 2 | real low: double reload, lock [concurrency] | — | Opus ✓ |
| 24 | #268 a hanging device never holds up Stop | fix | concurrency, capture | clear cause | +300 −69, 5 | Opus | 1 | medium → follow-up #269 (Core Audio query still blocks) [concurrency] | — | Opus ✓ |
| 25 | #260 one live-notes session per recording | feature | concurrency, runs | design | +738 −83, 9 | Opus | 2 | real: a stop race leaves an orphaned session [concurrency] | the race | Opus ✓ |
| 26 | #607 Yolo runs list | feature | UI, process lifecycle | spec | +912 −7, 18 | Opus, 3 msgs | 3 | real: pid reuse shows a dead run alive; per-row process lookup on Windows [concurrency] | pid reuse | Opus ✓ |
| 27 | #594 a call's faces are who joined | feature | capture data, UI | spec (issue) | +515 −29, 12 | Opus, 4 msgs | 3 | real low: status suffixes in names [edge] | — | Opus ✓ |
| 28 | #250 echo found during the call | feature | capture signal | design | +885 −56, 10 | Opus | 2 | low: a known limit not recorded in its case [test gap] | — | Opus ✓ |
| 29 | #263 a call's language is chosen | feature | capture, cross-area | design | +1002 −1232, 53 | Opus | 3 | real low [edge] | — | Opus ✓ |
| 30 | #189 an every-call term stays every-call | fix | persistence, migration | clear | +32 −12, 7 | Opus | 1 | risk: an old config migrates once; downgrade checked [migration] | the downgrade path | Opus ✓ |
| 31 | #64 Brain at scale | feature | store migration | design | +1758 −330, 11 | Opus | 2 | low: open waits only 10 s [concurrency] | — | Opus ✓ |
| 32 | #557 data kept for another kind asked again | fix | persistence | clear | +44 −7, 5 | Opus | 2 | real low: old-shaped data stamped [edge] | — | Opus ✓ |
| 33 | #145 hotspot split plus fixes | refactor + fixes | everything | mixed | +18683 −16336, 270 | Opus, 9 parallel reviews | 2 | real: stamp prefix compare, vacuous test, double load — all in the non-move parts [regression, test gap] | — | split ✗ |
| 34 | Split `main.rs` into CLI modules | refactor | CLI | clear, move-only | large | Opus | 1 | clean (line-by-line compare) | — | Sonnet ✗ |
| 35 | Move-only split of the ledger | refactor | ledger | clear | — | Opus | 1 | clean | — | Sonnet ✗ |
| 36 | #617 / #618 guides split into rules and reference | docs | guides | move + rule check | +607 −192, 11 | Opus | 2 | low; outdated rules fixed in a follow-up [design] | which text is a rule | split ✗ |
| 37 | #619 QA setup into `tests/` | chore | tests, config | clear | +85 −39, 7 | Opus | 2 | low wording | — | Sonnet ✗ |
| 38 | #609 autotest help text | test fix | tests | clear | +2 −1, 2 | Opus review | 1 | clean | — | Sonnet ✗ |
| 39 | #613 release-run test fixes | test fix | tests | clear | +5 −4, 4 | Opus review | 1 | low | — | Sonnet ✗ |
| 40 | #537 a table link cut in its cell | fix | UI CSS | clear | +5 −3, 3 | Opus | 2 | real low [edge] | — | Sonnet ✗ |
| 41 | #352 calendar card stays inside the grid | fix | UI layout | clear | +28 −15, 8 | Opus | 1 | real: card runs off a short window [edge] | — | Sonnet ✗ |
| 42 | #388 a time chip's turn stays highlighted | fix | UI state (Find, redraw) | clear | +9 −6, 4 | Opus | 1 | 2 real: highlight stuck after a jump [regression] | both | Opus ✓ |
| 43 | #392 / #396 search and settings keep the keyboard | fix | UI focus | clear | +3 −4; +9 −7 | Opus | 1 | clean | — | Opus ✓ |
| 44 | #286 prep header: how each person answered | feature | UI, data | design | +240 −99, 16 | Opus | 1 | escaped #331: page 121 000 px wide on hostile data [regression] | missed by Opus: not checked on hostile data | Opus ✓ |
| 45 | #365 default Today | feature | UI layout | design | +111 −31, 11 | Opus | 1 | escaped #367: columns clipped [regression] | missed by Opus: one window size | Opus ✓ |
| 46 | Write 18 cases for uncovered features (4 agents) | tests | tests | features named, no criteria | 18 cases | Sonnet | 1 + Opus review | real: expected results restate the code, bugs become expected [design]; 13 rewritten | what the user expects vs what the code does | Opus ✗ |
| 47 | Rewrite those cases by criteria (2 agents) | tests | tests | clear: criteria given | 13 cases | Sonnet | Opus check | clean | — | Sonnet ✓ |
| 48 | First case batches: threads, tasks, settings, hostile, state depth (6 agents) | tests | tests | features named, from the code | ~150 cases | Sonnet | later Opus rework | stale and clock-dependent cases fixed later (Opus, 113 tool calls) [test gap] | clock-dependent expectations | Opus ✗ |
| 49 | Link manual cases to features | mapping | tests, map | clear | map file | Sonnet | — | clean; flagged bundled features | — | Sonnet ✓ |
| 50 | Fix case wording from a run | test fix | tests | clear: files and wording named | 10 cases | Sonnet | — | clean; flagged mismatches | — | Sonnet ✓ |
| 51 | Running written cases and scenarios (152 runs) | run | tests | clear | — | Sonnet | — | FAILs triaged by the lead | — | Sonnet ✓ |
| 52 | Functional runs of a client plugin set (8 agents) | run + diagnosis | plugins, tests | clear cases | 78–186 tool calls | Opus | — | found real gaps (the served window never sends `meeting_soon`) | the diagnosis, not the run | split ✗ |
| 53 | Run reports (19) | format | tests | clear | — | Haiku | — | clean | — | Haiku ✓ |
| 54 | Docs and page lookups (13) | lookup | docs | clear | — | Haiku | — | clean | — | Haiku ✓ |
| 55 | Code surveys feeding test data (5 data-shape agents, 4 Explore) | survey | code, read-only | named question | read-only | Opus | — | facts used by the generator | — | Sonnet ✗ |
| 56 | Build the scenario data generator | tooling | tests, data | design | 123 tool calls | Opus | — | — | — | Opus ✓ |
| 57 | Rebuild scenarios to reuse assets (coordinator + 6) | tests design | tests | design | 34 msgs | Opus | — | cases out of order found [design] | stage order | Opus ✓ |
| 58 | Time rebuilt scenarios in the browser | run | tests | clear | 790 tool calls | Opus | — | — | — | Sonnet ✗ |
| 59 | #204 test data after 22:00 | fix | test data, clock | clear | +51 −32, 8 | Opus | 2 | changes requested [time edge] | the hour boundary | Opus ✓ |
| 60 | Rules PRs (#280, #534, #541, #545) and their reviews | rules | guides | judgment | 1–2 files | Opus | 1–3 | low wording; a rule an author can't test [design] | — | Opus ✓ |
| 61 | Small UI PR reviews (#386, #387, #389, #530, #553, …) | review | UI, plugins | clear | ≤ 100 lines | Opus | 1 | mostly none; speculative and not-a-bug | — | Opus ✓ |
| 62 | Competitor study and the audit of Echo against it (10 agents) | research | product | judgment synthesis | read-only | Opus | — | — | — | Opus ✓ |

## Factors

Only the factors the history supports, with the rows that show them.

1. **Security, permissions, the guard, strict routing, untrusted text or sends.** The costliest misses, found late and
   only by Opus reading rules against code: rows 1, 4, 5, 6, 8, 10, 12, 13. Even Opus missed one (#446 → #448) when the
   brief didn't name the boundary.
2. **Unknown cause, flaky, or clock-dependent.** The first fixes were the wrong design; reviews replaced the approach:
   rows 18, 19, 9, 59, 48 (late-evening and midnight failures: #364, #476, #593, #623, #628).
3. **UI state machine** (focus, hover, scroll, redraw, keyboard). Small diffs, many rounds, a new real bug per round:
   rows 17, 20, 42 (+9 lines, 2 real bugs), 43. Static layout (rows 40, 41) had one low bug each, which the review caught.
4. **Concurrency and process lifecycle** (watchers, reloads, sessions, pids, hangs): rows 23–26, 31.
5. **Persistence, formats, migration** (config, stores, saved records, downgrades): rows 30–32, 22.
6. **Design judgment: a new capability, several areas, owner choices**: rows 5, 6, 12, 21, 27–29, 56, 57.
7. **Writing tests decides what is expected.** Without acceptance criteria in the brief, Sonnet wrote cases from the
   code, so bugs became expected (rows 46, 48); with criteria, Sonnet's rewrite passed (row 47).
8. **Clear spec, mechanical, checkable**: move-only refactors, named wording fixes, running written cases, mapping,
   reports: rows 34, 35, 37–39, 49–51, 53, 54 — clean or low findings.
9. **Review versus implementation.** Reviews found real bugs in diffs that passed every gate, including tiny ones
   (rows 41, 42); size does not tell a review's worth.

Not a factor: **size alone.** +9 lines (row 42) and +123 (row 17) needed Opus; +18 000 lines of moves (row 33's move part,
row 34) were clean. Size only says whether a task should be split.

## The decision tree

Ask in order, from the brief, before starting; the first yes decides.

0. **Is it the lead's own part?** Planning, design choices, questions to the owner, the plan's Tests part, triage of
   review findings (real, speculative, not a bug), escalation and merge decisions stay with the lead, on the session's
   model. Delegate only work with its own brief: independent, parallel, or a wide read whose conclusion is enough.
1. **Does it touch security, permissions, the headless-run guard, run allowlists, strict routing, plugin grants or
   installs, sends, paths or text from outside reaching HTML or a model — or review or audit such work?** → Opus.
   (rows 1, 2, 4–6, 8, 10–16)
2. **Is it a review, re-review or audit?** → at least the author's model, and Opus for any review of code or rules
   (rows 41, 42, 61: real bugs in small diffs). A re-review goes to the same reviewer by message, keeping its context
   (45 such rounds). The brief names what to check: the boundary (routing, guard, grants), hostile data, the narrow
   and short window, Windows (rows 4, 44, 45, 17).
3. **Is the cause unknown, the behaviour flaky, clock- or timezone-dependent, or concurrent** (watchers, reloads,
   sessions, pids, hangs)? → Opus. (rows 18, 19, 23–26, 59)
4. **Does it need design judgment** — a new capability, two or more areas, an owner choice, a UI state machine (focus,
   hover, scroll, redraw, keyboard), or a persistence format or migration? → Opus. (rows 17, 20–22, 27–32, 42, 56, 57)
5. **Is it writing or changing tests or cases?** → Sonnet only when the brief gives each case's acceptance criteria and
   the data it runs on; otherwise Opus. (rows 46–48)
6. **Is it mechanical with a clear spec** — a known cause, named files, a stated expected result, checkable by the
   gates or a diff (move-only refactor, rename, a named wording or text fix, applying listed non-security review
   fixes, running written cases, mapping)? → Sonnet. (rows 34, 35, 37–41, 49–51, 58)
7. **Is it a read-only lookup or formatting** — fetch a page, look up docs, turn results into a report? → Haiku.
   (rows 53, 54)
8. **None fits or unsure** → one level up from the best guess; with no guess, Opus.

### Splitting a mixed task

When a task is part mechanical and part risky, and the risky part can be named in a brief with its own files: give the
mechanical part to Sonnet and the risky part to Opus, in that order, so the Opus part reviews what it builds on.
Otherwise the whole task goes on the stronger model. Examples: row 33 (the moves were clean, every real bug was in the
fixes), row 36 (moving text vs deciding what is a rule), row 7 (format moves vs per-plugin choices), row 52 (running
the cases vs diagnosing a FAIL).

### Escalation

- A cheaper model's work comes back with a real finding: first ask whether the brief lacked a fact (criteria, data,
  files). If it did, fix the brief and resend to the same agent (row 47). If the finding is design, security, a
  regression or an unknown cause, the next attempt goes one level up; never the same model twice on the same finding.
- The agent reports the spec doesn't hold (another cause, a choice to make): stop it; the lead decides, and the rest
  goes one level up.
- A low finding or a nit: the same agent fixes it by message.
- Two review rounds in a row that each find a new real bug (row 17: four) mean the design is wrong: the lead re-plans,
  asking the owner when it is a product choice (rows 5, 18).

## Counter-checks

The tree run over all 62 rows: 47 agree with what was used, 15 differ.

- **Cheaper than used (Opus → Sonnet), 9 rows: 34, 35, 37–41, 55, 58.** Their reviews were clean or low and the one
  real bug (row 41) was caught by the Opus review, which the tree keeps. The history suggests Sonnet would have fitted;
  it has no Sonnet implementation to prove it, so these are the first to watch.
- **Split instead of whole, 4 rows: 7, 33, 36, 52.** Row 33 supports it: all real bugs were in the non-move parts.
  Row 36 supports it: the follow-up fixed rule decisions, not moves. Rows 7 and 52 are untested.
- **Stronger than used (Sonnet → Opus), 2 rows: 46, 48.** The history supports the tree: 13 of 18 cases rewritten, and
  later rework of stale and clock-dependent cases. Row 47 shows the alternative — Sonnet with the criteria — works.
- **Right model, still missed (rows 4, 44, 45; #269 deferred knowingly).** The model was already the strongest; what
  was missing was in the brief. Hence the review rule in question 2: name the boundary, hostile data, window sizes and
  Windows.
- **Adjustments made while checking.** A size threshold was dropped (rows 17, 42 vs 34); UI state machine was added to
  question 4 (rows 17, 42 vs 40, 41); tests got their own question (rows 46–48); runs were split from diagnosis (rows
  51, 52); the review brief names its boundaries (rows 4, 44, 45).

## How to update

- Add a row when a task shows a factor the table lacks, or when a cheaper pick fails or a stronger one proves needless;
  cite the PR or issue number, never call content or real names.
- When a model is renamed or a new tier appears, change only the "Current models per class" table.
- When a row changes a branch of the tree, run the counter-check again and update its counts.
