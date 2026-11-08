#!/usr/bin/env python3
"""Wait for a condition of the run's Echo, with a timeout, instead of a fixed `sleep` in a case.

    tests/fixtures/wait.py <timeout s> <condition> [args]      (after `. <dir>/env`)

Conditions (the app's state from `echoctl status`; events from the run's .echo/events.jsonl, only those after the
count in $D/ev.start when that file exists, D the folder above ECHOCTL_HOME):
  app                  the app's watcher answers (status has watcher.pid)
  no-app               no watcher (the app has quit)
  armed                the watcher holds an arm (watcher.armed true)
  disarmed             no arm (status armed null and watcher.armed false)
  in-call              the watcher sees the call app in a call
  no-call              the watcher sees no call
  recording [suffix]   a recording runs (its folder ends with -<suffix> when given)
  not-recording        nothing records
  event <name> [n]     the event has come n times (default 1)
  file <glob>          a file matching the glob exists (a path under the run's folders)
  no-file <glob>       nothing matches the glob
  sh <command>         the shell command exits 0

It checks every second, prints what it waited for and how long it took, and exits 0; on the timeout it prints the
last state and exits 1 (the step fails with that output). Standard library only.
"""
import glob
import json
import os
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BIN = os.environ.get("ECHOCTL_BIN") or os.path.join(ROOT, "target", "debug", "echoctl")


def status():
    r = subprocess.run([BIN, "status"], capture_output=True, text=True)
    try:
        return json.loads(r.stdout)
    except ValueError:
        return {"error": (r.stderr or r.stdout).strip()}


def watcher():
    return status().get("watcher") or {}


def events():
    home = os.environ.get("ECHOCTL_HOME") or sys.exit("wait.py: events need ECHOCTL_HOME (`. <dir>/env` first)")
    start = 0
    try:
        start = int(open(os.path.join(os.path.dirname(home), "ev.start")).read().strip() or 0)
    except (OSError, ValueError):
        pass
    try:
        lines = open(os.path.join(home, ".echo", "events.jsonl"), encoding="utf-8").read().splitlines()[start:]
    except OSError:
        return []
    out = []
    for ln in lines:
        try:
            out.append(json.loads(ln).get("event"))
        except ValueError:
            pass
    return out


def check(what, args):
    if what == "app":
        return bool(watcher().get("pid"))
    if what == "no-app":
        return not status().get("watcher")
    if what == "armed":
        return watcher().get("armed") is True
    if what == "disarmed":
        s = status()
        return s.get("armed") is None and not (s.get("watcher") or {}).get("armed")
    if what == "in-call":
        return watcher().get("in_call") is True
    if what == "no-call":
        return not watcher().get("in_call")
    if what == "recording":
        rec = watcher().get("recording")
        return bool(rec) and (not args or rec.rstrip("/").endswith("-" + args[0]))
    if what == "not-recording":
        return not watcher().get("recording")
    if what == "event":
        return events().count(args[0]) >= (int(args[1]) if len(args) > 1 else 1)
    if what == "file":
        return bool(glob.glob(args[0]))
    if what == "no-file":
        return not glob.glob(args[0])
    if what == "sh":
        return subprocess.run(["bash", "-c", " ".join(args)]).returncode == 0
    sys.exit(__doc__)


def main(argv):
    if len(argv) < 3:
        sys.exit(__doc__)
    if "ECHOCTL_CONFIG_DIR" not in os.environ:
        sys.exit("wait.py: `. <dir>/env` first (without it `echoctl status` reads the real Echo)")
    timeout, what, args = float(argv[1]), argv[2], argv[3:]
    t0 = time.monotonic()
    while True:
        if check(what, args):
            print(f"waited {time.monotonic() - t0:.1f} s for {what} {' '.join(args)}".rstrip())
            return 0
        if time.monotonic() - t0 >= timeout:
            last = events()[-8:] if what == "event" else status()
            print(f"timed out after {timeout:.0f} s waiting for {what} {' '.join(args)}; last: {json.dumps(last)[:2000]}")
            return 1
        time.sleep(1)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
