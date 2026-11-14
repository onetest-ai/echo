#!/usr/bin/env bash
# Install the manual-qa agent team (arozumenko/sdlc-skills) on this machine, inside tests/: only a Claude Code
# session started in tests/ (`cd tests && claude`) loads the agents and the Playwright MCP (tests/.mcp.json).
#
# The installer writes per-machine, git-ignored files into tests/ (.claude/agents, .claude/skills, .claude/hooks,
# .claude/settings.json, .agents/manual-qa). This script then:
#   - keeps tests/CLAUDE.md as it was and drops the AGENTS.md the installer adds (Echo's rules are in app_profile.md),
#   - moves the agents' case folder to cases/ and points them at app_profile.md (both in tests/),
#   - tells every role that app_profile.md wins over its own rules.
# Run it again after an update of the bundle: `tests/install.sh`.
set -euo pipefail

tests="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/tests"
cd "$tests"

# The installer appends its team text to CLAUDE.md and AGENTS.md; keep CLAUDE.md as it is, leave no AGENTS.md.
keep="$(mktemp -d)"
cp CLAUDE.md "$keep"/
trap 'cp "$keep"/CLAUDE.md .; rm -f AGENTS.md; rm -rf "$keep"' EXIT

npx -y github:arozumenko/sdlc-skills init --factory manual-qa --target claude --mcp playwright --yes --update

# Paths (from tests/, the session's folder): tasks/<suite>/ → cases/<suite>/; the profile → app_profile.md;
# reports/ is already tests/reports/.
agents=(.claude/agents/{app-profiler,test-sizer,test-author,test-run-lead,test-runner,test-reporter,qa-auditor})
for a in "${agents[@]}"; do
  for f in "$a"/*.md; do
    perl -pi -e '
      s{\.agents/manual-qa/app_profile\.md}{app_profile.md}g;
      s{(^|[^A-Za-z0-9_./-])tasks/}{$1cases/}g;
    ' "$f"
  done
  rules="$a/RULES.md"
  if ! grep -q "Echo:" "$rules"; then
    printf '\n%s\n' "- **Echo:** \`app_profile.md\` (in tests/, this session's folder) holds Echo's own rules (test data, starting the app, CLI steps, cases a person runs); its paths are from the repo root (\`..\`). Where it differs from these rules, it wins." >> "$rules"
  fi
done

# Echo's own rules on top of the agents (times per stage, …): tests/agent-rules.sh, also run by serve.sh up.
"$tests/agent-rules.sh"

# The hooks give each role `.agents/manual-qa/app_profile.md`: the same file as tests/app_profile.md.
mkdir -p .agents/manual-qa
ln -sfn ../../app_profile.md .agents/manual-qa/app_profile.md

left="$(grep -lE '(^|[^A-Za-z0-9_./-])tasks/|\.agents/manual-qa/app_profile' "${agents[@]/%//AGENT.md}" "${agents[@]/%//RULES.md}" 2>/dev/null || true)"
if [ -n "$left" ]; then
  echo "warning: paths not moved in:" >&2
  echo "$left" >&2
fi
echo "manual-qa installed in tests/: cases in tests/cases, reports in tests/reports; start QA sessions there (cd tests && claude)"
