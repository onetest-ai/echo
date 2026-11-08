#!/usr/bin/env bash
# Source files stay under 1,500 lines (architecture.md, size limits): Rust counted up to its `#[cfg(test)] mod tests`,
# the window's parts (ui/app/*.js, ui/css/*.css) whole; vendored and generated files aren't looked at. A file
# over the limit today is listed in scripts/size-ceilings.txt with its size then: it may shrink, never grow.
set -euo pipefail
cd "$(dirname "$0")/.."
PY=python3; command -v python3 >/dev/null && python3 -c "" 2>/dev/null || PY=python  # Git Bash on Windows
"$PY" - <<'PY'
import glob, sys
from pathlib import Path
LIMIT = 1500
ceil = {}
for k, l in enumerate(open("scripts/size-ceilings.txt", encoding="utf-8"), 1):
    f = l.split("#", 1)[0].split()
    if not f:
        continue
    if len(f) != 2 or not f[1].isdigit():
        sys.exit(f"size-check: scripts/size-ceilings.txt line {k}: expected `<path> <lines>`, got {l.strip()!r}")
    ceil[f[0]] = int(f[1])
def size(p):
    lines = open(p, encoding="utf-8").read().splitlines()
    if p.endswith(".rs"):
        for i, l in enumerate(lines[:-1]):
            if l.strip() == "#[cfg(test)]" and lines[i + 1].startswith("mod tests"):
                return i
    return len(lines)
files = [Path(p).as_posix() for p in glob.glob("crates/echoctl/src/**/*.rs", recursive=True) + glob.glob("crates/echoctl/ui/app/*.js") + glob.glob("crates/echoctl/ui/css/*.css")]
bad, smaller = [], []
for p in sorted(files):
    n, c = size(p), ceil.get(p)
    if c is None and n > LIMIT:
        bad.append(f"{p}: {n} lines, over {LIMIT}: split it first, in a move-only commit (architecture.md, size limits)")
    elif c is not None and n > c:
        bad.append(f"{p}: {n} lines, over its ceiling {c}: split it, or move code out, before it grows (architecture.md, size limits)")
    elif c is not None and n < c:
        smaller.append(f"{p}: {n} lines, ceiling {c}: lower it in scripts/size-ceilings.txt" + (" (or drop it: under the limit)" if n <= LIMIT else ""))
for p in ceil:
    if p not in files:
        bad.append(f"scripts/size-ceilings.txt lists {p}, which doesn't exist: drop it")
for m in smaller:
    print("note:", m)
for m in bad:
    print("size-check:", m, file=sys.stderr)
sys.exit(1 if bad else 0)
PY
echo "size-check: ok"
