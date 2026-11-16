#!/usr/bin/env python3
"""The verifier's answer for a test (#463): every claim a thread's ledger waits on gets one verdict.

    tests/fixtures/ledger/verify.py <thread> supports|weak|wrong [--covers]

`--covers`: only the covering mentions ("Looks done", their claim "covers it…"), not the other claims the data waits
on. Runs `echoctl project ledger <thread> verify-prep`, writes a verdict for each packet into the run's data folder and
applies it (`verify-apply`): what the sync's verifier would do, without a model. Run after `. <dir>/env`
(ECHOCTL_HOME is the run's data); `ECHOCTL_BIN` (default `target/debug/echoctl`) is the build under test."""
import json
import os
import subprocess
import sys

thread, verdict = sys.argv[1], sys.argv[2]
only_covers = "--covers" in sys.argv[3:]
if verdict not in ("supports", "weak", "wrong"):
    sys.exit("verdict: supports, weak or wrong")
bin_ = os.environ.get("ECHOCTL_BIN", "target/debug/echoctl")
packets = json.loads(subprocess.run([bin_, "project", "ledger", thread, "verify-prep"], check=True, capture_output=True, text=True).stdout)
if only_covers:
    packets = [p for p in packets if str(p.get("claimed", "")).startswith("covers it")]
out = os.path.join(os.environ["ECHOCTL_HOME"], "..", "verdicts.json")
with open(out, "w") as f:
    json.dump([{"mid": p["mid"], "verdict": verdict, "reason": "test"} for p in packets], f)
r = subprocess.run([bin_, "project", "ledger", thread, "verify-apply", "--file", out], check=True, capture_output=True, text=True)
print(json.dumps({"verdicts": len(packets), "applied": json.loads(r.stdout or "null")}))
