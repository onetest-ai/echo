#!/usr/bin/env bash
# Install the manual-qa agent team (arozumenko/sdlc-skills) on this machine and point it at Echo's tests.
#
# The installer writes per-machine, git-ignored files (.claude/agents, .claude/skills, .claude/hooks,
# .agents/manual-qa). This script then:
#   - keeps AGENTS.md and CLAUDE.md as they were (the installer appends its team text; Echo's rules are in tests),
#   - moves the agents' paths to tests (cases/, reports/) and links the app profile there,
#   - tells every role that tests/app_profile.md wins over its own rules.
# Run it again after an update of the bundle: `tests/install.sh`.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"
tests="tests"

# The installer appends its team text to AGENTS.md and CLAUDE.md; keep both as they are.
keep="$(mktemp -d)"
cp AGENTS.md CLAUDE.md "$keep"/
trap 'cp "$keep"/AGENTS.md "$keep"/CLAUDE.md .; rm -rf "$keep"' EXIT

npx -y github:arozumenko/sdlc-skills init --factory manual-qa --target claude --mcp playwright --yes --update

# Paths: tasks/<suite>/ → tests/cases/<suite>/, reports/ → tests/reports/, the profile → tests.
agents=(.claude/agents/{app-profiler,test-sizer,test-author,test-run-lead,test-runner,test-reporter,qa-auditor})
for a in "${agents[@]}"; do
  for f in "$a"/*.md; do
    perl -pi -e '
      s{\.agents/manual-qa/app_profile\.md}{tests/app_profile.md}g;
      s{(^|[^A-Za-z0-9_./-])tasks/}{$1tests/cases/}g;
      s{(^|[^A-Za-z0-9_./-])reports/}{$1tests/reports/}g;
    ' "$f"
  done
  rules="$a/RULES.md"
  if ! grep -q "Echo:" "$rules"; then
    printf '\n%s\n' "- **Echo:** \`tests/app_profile.md\` holds Echo's own rules (test data, starting the app, CLI steps, cases a person runs). Where it differs from these rules, it wins." >> "$rules"
  fi
done

# The run-metrics hooks write next to the run reports: tests/reports/metrics (not a reports/ folder at the root).
for f in .claude/hooks/manual-qa/*.mjs; do
  perl -pi -e "s{join\(PROJECT_DIR, 'reports'}{join(PROJECT_DIR, 'tests', 'reports'}g" "$f"
done

# The hooks give each role `.agents/manual-qa/app_profile.md`: the same file as tests/app_profile.md.
mkdir -p .agents/manual-qa
ln -sfn "../../$tests/app_profile.md" .agents/manual-qa/app_profile.md

left="$(grep -lE '(^|[^A-Za-z0-9_./-])(tasks|reports)/|\.agents/manual-qa/app_profile' "${agents[@]/%//AGENT.md}" "${agents[@]/%//RULES.md}" 2>/dev/null || true)"
if [ -n "$left" ]; then
  echo "warning: paths not moved in:" >&2
  echo "$left" >&2
fi
# The pre-push hook (functional map up to date before a push) lives in the repo; point git at it.
git config core.hooksPath .githooks

echo "manual-qa installed for Echo: cases in $tests/cases, reports in $tests/reports"
