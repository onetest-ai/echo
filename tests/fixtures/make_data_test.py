#!/usr/bin/env python3
"""make_data.py's own checks: the binary it builds the data with (#418)."""
import importlib.util
import os
import unittest
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


if __name__ == "__main__":
    unittest.main()
