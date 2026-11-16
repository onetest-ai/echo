#!/usr/bin/env bash
# Install the manual-qa agent team (arozumenko/sdlc-skills) on this machine, inside tests/: only a Claude Code
# session started in tests/ (`cd tests && claude`) loads the agents and the Playwright MCP (tests/.mcp.json).
#
# The installer writes per-machine, git-ignored files into tests/ (.claude/agents, .claude/skills, .claude/hooks,
# .claude/settings.json, .agents/manual-qa). This script then:
#   - keeps tests/CLAUDE.md as it was and drops the AGENTS.md the installer adds (Echo's rules are in app_profile.md),
#   - writes the bundle's paths as absolute ones in tests/ (cases/, reports/, app_profile.md): Echo's own files write
#     paths from the repo root, so every path a tool gets is absolute and none depends on the session's folder,
#   - tells every role that app_profile.md wins over its own rules,
#   - warns when an older install at the repo root is still there (a session in tests/ loads the parent's agents too).
# Run it again after an update of the bundle: `tests/install.sh`.
set -euo pipefail

tests="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)/tests"
cd "$tests"

# The installer appends its team text to CLAUDE.md and AGENTS.md; keep CLAUDE.md as it is, leave no AGENTS.md.
keep="$(mktemp -d)"
cp CLAUDE.md "$keep"/
trap 'cp "$keep"/CLAUDE.md .; rm -f AGENTS.md; rm -rf "$keep"' EXIT

npx -y github:arozumenko/sdlc-skills init --factory manual-qa --target claude --mcp playwright --yes --update

# Paths: tasks/<suite>/ → <tests>/cases/<suite>/, reports/ → <tests>/reports/, the profile → <tests>/app_profile.md.
agents=(.claude/agents/{app-profiler,test-sizer,test-author,test-run-lead,test-runner,test-reporter,qa-auditor})
files=()
while IFS= read -r f; do files+=("$f"); done < <(find "${agents[@]}" .agents/manual-qa/knowledge \
  .claude/skills/playwright-testing .claude/skills/mobile-testing -name '*.md' 2>/dev/null)
T="$tests" perl -pi -e '
  s{\.agents/manual-qa/app_profile\.md}{$ENV{T}/app_profile.md}g;
  s{(^|[^A-Za-z0-9_./-])tasks/}{$1$ENV{T}/cases/}g;
  s{(^|[^A-Za-z0-9_./-])reports/}{$1$ENV{T}/reports/}g;
' "${files[@]}"
for a in "${agents[@]}"; do
  rules="$a/RULES.md"
  if ! grep -q "Echo:" "$rules"; then
    printf '\n%s\n' "- **Echo:** \`$tests/app_profile.md\` holds Echo's own rules (test data, starting the app, CLI steps, cases a person runs). Its paths, the cases' and the scenarios' are from the repo root (\`${tests%/tests}\`): give every tool an absolute path, and start each Bash call with \`cd ${tests%/tests}\`. Where it differs from these rules, it wins." >> "$rules"
  fi
done

# Echo's own rules on top of the agents (times per stage, …): tests/agent-rules.sh, also run by serve.sh up.
"$tests/agent-rules.sh"

# The hooks give each role `.agents/manual-qa/app_profile.md`: the same file as tests/app_profile.md.
mkdir -p .agents/manual-qa
ln -sfn ../../app_profile.md .agents/manual-qa/app_profile.md

left="$(grep -lE '(^|[^A-Za-z0-9_./-])(tasks|reports)/|\.agents/manual-qa/app_profile' "${files[@]}" 2>/dev/null || true)"
if [ -n "$left" ]; then
  echo "warning: paths not moved in:" >&2
  echo "$left" >&2
fi

# An older install at the repo root: a session in tests/ loads the parent's .claude/agents as well, so it must go.
root="${tests%/tests}"
if [ -d "$root/.claude/agents/test-run-lead" ]; then
  old=(.claude/agents/{app-profiler,test-sizer,test-author,test-run-lead,test-runner,test-reporter,qa-auditor}
       .claude/hooks/manual-qa .claude/hooks/sdlc-skills .agents/manual-qa)
  for d in .claude/skills/*/; do d="${d%/}"; [ -e "$root/$d" ] && old+=("$d"); done
  echo "warning: the old install at the repo root is still there; a session in tests/ would load its agents too." >&2
  echo "Remove it (from $root):" >&2
  echo "  cd '$root' && rm -rf ${old[*]}" >&2
  echo "and drop its hooks (\"_factory\") from .claude/settings.json, keeping the rest:" >&2
  echo "  python3 -c 'import json;p=\".claude/settings.json\";d=json.load(open(p));h=d.get(\"hooks\",{});[h.__setitem__(k,[e for e in v if \"_factory\" not in e]) for k,v in list(h.items())];[h.pop(k) for k in [k for k,v in h.items() if not v]];json.dump(d,open(p,\"w\"),indent=2)'" >&2
fi
echo "manual-qa installed in tests/: cases in tests/cases, reports in tests/reports; start QA sessions there (cd tests && claude)"
