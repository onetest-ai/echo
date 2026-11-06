#!/usr/bin/env bash
# Serve the window on fresh made-up data for one scenario: `tests/fixtures/serve.sh up <dir> <port>`, and
# `tests/fixtures/serve.sh down <dir>` after it. <dir> must not exist yet (or be empty); it gets the data
# (make-data.sh), the server's pid and its log. Prints the base_url and the env file for CLI steps.
set -euo pipefail

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
cmd="${1:?usage: serve.sh up <dir> <port> | down <dir>}"
dir="${2:?usage: serve.sh up <dir> <port> | down <dir>}"

case "$cmd" in
  up)
    port="${3:?usage: serve.sh up <dir> <port>}"
    "$root/tests/fixtures/make-data.sh" "$dir" > /dev/null
    dir="$(cd "$dir" && pwd)"
    (
      . "$dir/env"
      cd "$root"
      ECHOCTL_SERVE_IPC=1 nohup target/debug/echoctl window --serve "$port" > "$dir/serve.log" 2>&1 &
      echo $! > "$dir/pid"
    )
    for _ in $(seq 20); do
      curl -s -o /dev/null "http://127.0.0.1:$port/" && break
      sleep 0.5
    done
    curl -s -o /dev/null "http://127.0.0.1:$port/" || { echo "the window didn't start: see $dir/serve.log" >&2; exit 1; }
    echo "base_url=http://127.0.0.1:$port env=$dir/env"
    ;;
  down)
    [ -f "$dir/pid" ] && kill "$(cat "$dir/pid")" 2> /dev/null || true
    echo "stopped $dir"
    ;;
  *) echo "usage: serve.sh up <dir> <port> | down <dir>" >&2; exit 1 ;;
esac
