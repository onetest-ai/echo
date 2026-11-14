#!/usr/bin/env python3
"""What's new as seen up to a version, in a run's config folder (whats-new-seen.json, `whats_new.rs`):
`python3 tests/fixtures/whats_new_seen.py <running|previous|none|VERSION>` after `. <dir>/env`.

  running   this build's version: nothing to show (test data's default, make_data.py --whats-new-seen)
  previous  the entry before this build's in crates/echoctl/whats-new.json: an update to this version
  none      no file: a fresh install, or an Echo from before What's new
  VERSION   e.g. 0.0.1: every entry after it

The window shows what's new at its next load: navigate it away first (about:blank) when it is open."""
import json
import os
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def write(config, bin, seen):
    path = os.path.join(config, "whats-new-seen.json")
    if seen == "none":
        if os.path.exists(path):
            os.remove(path)
        return
    running = subprocess.run([bin, "--version"], capture_output=True, text=True, check=True).stdout.split()[-1]
    if seen == "running":
        seen = running
    elif seen == "previous":
        # The entry before the running version's (an entry written ahead of its release is skipped).
        versions = [e["version"] for e in json.load(open(os.path.join(ROOT, "crates", "echoctl", "whats-new.json")))]
        older = versions[versions.index(running) + 1:] if running in versions else []
        if not older:
            sys.exit(f"previous: whats-new.json has no entry before {running}'s")
        seen = older[0]
    with open(path, "w") as f:
        json.dump({"version": seen}, f, indent=2)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    config, home = os.environ.get("ECHOCTL_CONFIG_DIR"), os.environ.get("ECHOCTL_HOME")
    if not config or not home or os.path.commonpath([config, home]) in ("/", os.path.expanduser("~")):
        sys.exit("run it after `. <dir>/env` (ECHOCTL_CONFIG_DIR and ECHOCTL_HOME of the run's test data)")
    bin = os.environ.get("ECHOCTL_BIN") or os.path.join(ROOT, "target", "debug", "echoctl.exe" if os.name == "nt" else "echoctl")
    write(config, bin, sys.argv[1])
    print(f"what's new seen: {sys.argv[1]}")
