#!/usr/bin/env bash
# Validate, test and type-check the Echo mod (plugin/hooks) the way the engine reads it, and test
# the window's calendar dates.
# Type-checks against the engine's own declarations: the copy it lays beside a hot-reloaded mod
# (plugin/.claude-plugin/types) if present, else the one the plugin-authoring skill wrote.
set -euo pipefail
cd "$(dirname "$0")/.."
claude plugin validate plugin
claude plugin test plugin
# The calendar's days and hours across the days the clocks change.
node crates/echoctl/ui/calendar-dates.test.mjs
types="plugin/.claude-plugin/types/claude-code/index.d.ts"
if [[ ! -f "$types" ]]; then
  types=$(ls -t /private/tmp/claude-*/bundled-skills/*/*/plugin-authoring/types/claude-code.d.ts 2>/dev/null | head -1 || true)
fi
if [[ -z "${types:-}" ]]; then
  echo "skip tsc: no engine type declarations found (load the plugin-authoring skill once)"; exit 0
fi
tmp=$(mktemp -d)
cat > "$tmp/tsconfig.json" <<JSON
{ "compilerOptions": { "target": "es2023", "lib": ["es2023"], "types": [], "module": "esnext",
  "moduleResolution": "bundler", "strict": true, "noUncheckedIndexedAccess": true, "noEmit": true,
  "skipLibCheck": true, "jsx": "react", "jsxFactory": "h", "jsxFragmentFactory": "Fragment" },
  "files": ["$types"], "include": ["$PWD/plugin/hooks", "$PWD/plugin/types"] }
JSON
npx -y -p typescript@5.6.3 tsc -p "$tmp/tsconfig.json"
echo "tsc: clean"
