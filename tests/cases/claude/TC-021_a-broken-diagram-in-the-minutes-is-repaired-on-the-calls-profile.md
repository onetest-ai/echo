---
id: TC-021
title: A minutes diagram mermaid can't parse is repaired on the call's profile and drawn; one that still fails shows its source with the error, not the Syntax error bomb
priority: medium
type: functional
requirements: [F-MIN-22]
module: claude
run: agent
platform: any
tags: [minutes, diagrams, window]
---

# TC-021: A broken diagram in the minutes is repaired on the call's profile

**Module:** Minutes | **Priority:** Medium | **Type:** Functional

## Preconditions
- The window served on fake-AI test data: `tests/fixtures/serve.sh up <dir> <port> --ai fake`, at `{{base_url}}`; CLI steps run in Bash after `. <dir>/env`; `C="$ECHOCTL_HOME/calls/<day>_140000-globex-pilot-kickoff"` (it has minutes)
- The fake Claude Code answers the repair: `printf 'You fix one Mermaid diagram\t{"type":"result","subtype":"success","is_error":false,"result":"flowchart TD\\n  A[\\"Budget (Q3)\\"] --> B[\\"Pilot (Globex)\\"]"}\n' >> <dir>/fake-answers.tsv`

## Steps

| # | Action | Expected Result |
|---|--------|----------------|
| 1 | `printf '\n## Plan\n\n```mermaid\nflowchart TD\n  A[Budget (Q3)] --> B[Pilot (Globex)]\n```\n' >> "$C/notes.md"`; open `{{base_url}}/#call/<that call>/minutes` | The Minutes tab opens |
| 2 | Wait 6 s; `tail -5 "$C/notes.md"; grep "diagram 1 repaired" <dir>/serve.log` | The block now reads `A["Budget (Q3)"] --> B["Pilot (Globex)"]`, the rest of `notes.md` unchanged; the log says "diagram 1 repaired" |
| 3 | Open `{{base_url}}/minutes/<that call>`; scroll to "Plan" | The diagram is drawn (two boxes, "Budget (Q3)" → "Pilot (Globex)"); no "Syntax error" anywhere |
| 4 | Remove the fake answer (`sed -i '' '/You fix one Mermaid diagram/d' <dir>/fake-answers.tsv`); append a second broken one (`printf '\n## Risks\n\n```mermaid\nflowchart LR\n  R1[Data (EU)] --> R2[Legal sign-off]\n```\n' >> "$C/notes.md"`); open `{{base_url}}/#call/<that call>/minutes` again; wait 6 s; `grep "diagram 2" <dir>/serve.log; tail -4 "$C/notes.md"` | The log says "diagram 2 not repaired: …" (the fake refuses); the block in `notes.md` is unchanged |
| 5 | Open `{{base_url}}/minutes/<that call>`; scroll to "Risks" | Its box shows "This diagram couldn't be drawn: Parse error on line 2" above the diagram's source; no "Syntax error" bomb; the "Plan" diagram still drawn |
| 6 | Open `{{base_url}}/minutes/<that call>?theme=dark`, then `?theme=light` | Both: the "Plan" diagram drawn (dark, then light colours), "Risks" shown as its source with the reason; no "Syntax error" bomb (a chosen theme keeps the page's diagram script inside its policy) |

## Expected Final State
A broken diagram is repaired once on the call's profile and drawn; one that can't be shows its source with the reason.

## Teardown
- `tests/fixtures/serve.sh down <dir>`

## Notes
- Only that diagram and mermaid's error go to the model (`diagrams::repair`, its Rust test checks the prompt); the call's other text doesn't.
- A diagram is numbered as the page draws it (the same Markdown reading); one inside a quote is drawn but never rewritten.
- A diagram is tried once per app session: a repair that fails again isn't repaired again, so a page reload doesn't loop.
- The sent copy of the minutes page (the file) never talks to the window: there a broken diagram only shows its source with the reason.
