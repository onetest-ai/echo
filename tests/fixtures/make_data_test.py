#!/usr/bin/env python3
"""make_data.py's own checks: the binary it builds the data with (#418), and builds as of other times (--now, #550)."""
import importlib.util
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
                for t, code, out in pool.map(build, times):
                    with self.subTest(now=t.isoformat(timespec="minutes")):
                        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
