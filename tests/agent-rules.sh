#!/usr/bin/env bash
# Echo's own rules on top of the installed manual-qa agents (tests/.claude/agents/, git-ignored): added once, kept across a
# reinstall of the bundle. Run by tests/install.sh and by tests/fixtures/serve.sh up, so a run never starts without them.
set -euo pipefail
cd "$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/tests"
add() { # <agent> <marker> <rule>
  local f=".claude/agents/$1/RULES.md"
  [ -f "$f" ] || return 0
  grep -q "$2" "$f" || printf '\n%s\n' "$3" >> "$f"
}
# Times per stage (tests/app_profile.md → Scenarios): the runner measures each stage, the reporter prints it.
add test-runner "duration_ms per stage" "- **Echo, duration_ms per stage:** read \`python3 -c 'import time;print(int(time.time()*1000))'\` before a stage's first step and after its teardown; put the difference in the stage's \`duration_ms\` (and the scenario's, first start to last end). Measured, never estimated."
add test-reporter "duration_ms per stage" "- **Echo, duration_ms per stage:** the Results table has a Time column (each stage's \`duration_ms\`), and a section \"Time per Stage\" after it gives each scenario's time and the slowest 5 stages (stage, status, time, share of the run). A missing time is —."
