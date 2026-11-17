#!/usr/bin/env python3
"""make_data.py's own checks: the binary it builds the data with (#418), and builds as of other times (--now, #550)."""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timedelta
from unittest import mock

spec = importlib.util.spec_from_file_location("make_data", os.path.join(os.path.dirname(os.path.abspath(__file__)), "make_data.py"))
md = importlib.util.module_from_spec(spec)
spec.loader.exec_module(md)


class Binary(unittest.TestCase):
    def test_without_echoctl_bin_it_is_the_file_cargo_builds_on_this_os(self):
        env = {k: v for k, v in os.environ.items() if k != "ECHOCTL_BIN"}
        with mock.patch.dict(os.environ, env, clear=True):
            d = md.Data("unused", "small")
        name = "echoctl.exe" if os.name == "nt" else "echoctl"
        self.assertEqual(d.bin, os.path.join(md.ROOT, "target", "debug", name))

    def test_echoctl_bin_wins(self):
        with mock.patch.dict(os.environ, {"ECHOCTL_BIN": "/somewhere/echoctl"}):
            self.assertEqual(md.Data("unused", "small").bin, "/somewhere/echoctl")


class Git(unittest.TestCase):
    def test_git_under_the_data_env_reads_the_tests_own_settings_and_commits_as_the_made_up_user(self):
        """#707: never the user's ~/.gitconfig, ~/.config/git or GIT_AUTHOR_* — a commit on test data is Sam Rivera's."""
        real = {"GIT_AUTHOR_NAME": "Real Person", "GIT_AUTHOR_EMAIL": "real@person.example",
                "GIT_COMMITTER_NAME": "Real Person", "GIT_COMMITTER_EMAIL": "real@person.example"}
        with tempfile.TemporaryDirectory() as tmp, mock.patch.dict(os.environ, real):
            base = os.path.realpath(tmp)
            d = md.Data(base, "small")
            md.setup_env(d)
            for k in ("GIT_CONFIG_GLOBAL", "XDG_CONFIG_HOME"):
                self.assertTrue(d.env[k].startswith(base + os.sep), d.env[k])
            # A shell that sources <dir>/env (serve.sh, a case's step) over a user's identity variables.
            r = subprocess.run(["/bin/sh", "-c", '. "$1/env" && cd "$1" && git init -q repo && cd repo && git commit -q --allow-empty -m x'
                                ' && git log -1 --format="%an <%ae>|%cn <%ce>" && printf "%s|%s\\n" "$GIT_CONFIG_GLOBAL" "$XDG_CONFIG_HOME"', "sh", base],
                               capture_output=True, text=True, env=os.environ)
            self.assertEqual(r.returncode, 0, r.stderr)
            ident, cfg = r.stdout.strip().split("\n")
            self.assertEqual(ident, "Sam Rivera <sam@initech.example>|Sam Rivera <sam@initech.example>")
            self.assertEqual(cfg, f"{base}/gitconfig|{base}/xdg-config")
            # What make_data itself runs with: the same, the user's identity variables gone.
            for k in real:
                self.assertNotIn(k, d.env)


BIN = md.Data("unused", "standard").bin


@unittest.skipUnless(os.access(BIN, os.X_OK), f"no {BIN}: build first (cargo build -p echoctl) to check builds as of other times")
class OtherClocks(unittest.TestCase):
    def test_the_standard_set_builds_and_passes_its_own_checks_as_of_any_hour_and_day(self):
        """--now at the day's first and last hours, midday, yesterday and tomorrow: whatever the real clock reads."""
        day = datetime.now().replace(second=0, microsecond=0)
        times = [day.replace(hour=6, minute=0), day.replace(hour=12, minute=0), day.replace(hour=22, minute=50),
                 (day - timedelta(days=1)).replace(hour=15, minute=0), (day + timedelta(days=1)).replace(hour=9, minute=0)]
        script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "make_data.py")
        with tempfile.TemporaryDirectory() as tmp:
            def build(t):
                r = subprocess.run([sys.executable, script, os.path.join(tmp, t.strftime("%Y%m%d%H%M")), "--set", "standard",
                                    "--now", t.isoformat(timespec="minutes")], capture_output=True, text=True)
                return t, r.returncode, (r.stderr or r.stdout).strip()[-400:]
            with ThreadPoolExecutor(len(times)) as pool:
                built = list(pool.map(build, times))
            for t, code, out in built:
                with self.subTest(now=t.isoformat(timespec="minutes")):
                    self.assertEqual(code, 0, out)
            # The day keeps its shape at any hour: the same meetings to come at once (The day's choices, S-24 stage 48),
            # none joined by one squeezed in late in the evening.
            shapes = {t.isoformat(timespec="minutes"): at_once(os.path.join(tmp, t.strftime("%Y%m%d%H%M")), t) for t, code, _ in built if code == 0}
            first = next(iter(shapes.values()), None)
            for t, shape in shapes.items():
                with self.subTest(shape_at=t):
                    self.assertEqual(shape, first)


def at_once(base, now):
    """The accepted meetings still to come at `now` in clusters of ones at once, as The day's choices groups them
    (app.js dayChoices): each cluster's subjects, sorted."""
    with open(os.path.join(base, "home", ".echo", "agenda.json"), encoding="utf-8") as f:
        agenda = json.load(f)
    t = lambda s: datetime.fromisoformat(s).astimezone()
    now = now.astimezone()
    going = sorted((m for m in agenda if m.get("response") not in ("notResponded", "tentativelyAccepted", "declined") and t(m.get("end") or m["start"]) > now),
                   key=lambda m: (t(m["start"]), -(t(m.get("end") or m["start"]) - t(m["start"]))))
    out, cur, end = [], [], None
    for m in going:
        if cur and t(m["start"]) >= end:
            out.append(sorted(cur))
            cur = []
        if not cur:
            end = t(m.get("end") or m["start"])
        cur.append(m["subject"])
        end = max(end, t(m.get("end") or m["start"]))
    if cur:
        out.append(sorted(cur))
    return out


if __name__ == "__main__":
    unittest.main()
