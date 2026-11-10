#!/usr/bin/env bash
# Serve the window on fresh made-up data for one scenario: `tests/fixtures/serve.sh up <dir> <port> [--set SET]`,
# and `tests/fixtures/serve.sh down <dir>` after it. <dir> must not exist yet (or be empty); it gets the data
# (make_data.py; SET is small, the default, standard or hostile: tests/fixtures/README.md), the server's pid and its
# log. Prints the base_url and the env file for CLI steps.
set -euo pipefail

root="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
usage="usage: serve.sh up <dir> <port> [--set small|standard|hostile] [--ai fake|local|virtual] [--now TIME] | down <dir>"
cmd="${1:?$usage}"
dir="${2:?$usage}"

case "$cmd" in
  up)
    port="${3:?$usage}"
    # The agents' Echo rules (stage times, …) before any run: a reinstall or a new machine can't miss them.
    "$root/tests/agent-rules.sh"
    # Anything after the port goes to make_data.py: --set SET (small by default), --now TIME.
    python3 "$root/tests/fixtures/make_data.py" "$dir" "${@:4}" > /dev/null
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
    # `--ai virtual`: the virtual model started with the data.
    [ -f "$dir/model/pid" ] && kill "$(cat "$dir/model/pid")" 2> /dev/null || true
    echo "stopped $dir"
    ;;
  *) echo "$usage" >&2; exit 1 ;;
esac
