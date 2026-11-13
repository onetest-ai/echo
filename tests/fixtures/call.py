#!/usr/bin/env python3
"""A test call fed from files: no device opened, no sound played (after `. <dir>/env`).

    tests/fixtures/call.py start <conversation> [--for SECONDS | --hold] [--meet [--title TEXT] [--meet-app]]
                                                                             a call starts now; prints what it set up
    tests/fixtures/call.py end                                               hang up now
    tests/fixtures/call.py status                                            in a call or not, and since when

The run's env (tests/fixtures/make_data.py) points a debug build's sources at files
(crates/echoctl/src/audio/testfeed.rs): ECHOCTL_TEST_ME_WAV is the mic track, ECHOCTL_TEST_REMOTE_WAV the call app's
audio, and ECHOCTL_TEST_CALL_FILE stands for the call app being in a call: Echo's watcher sees a call while that file
exists, which started at the file's modification time. `--meet` makes it a call in Chrome instead of Teams (Google
Meet), and `--title` the title of Chrome's window (none: no window, so only a calendar meeting with a Meet link makes
it a Meet call); `--meet-app` has Google Meet installed as an app from Chrome open (`meet_app=1`). `start` puts the conversation's two tracks
(tests/fixtures/audio, built once per run into <dir>/conv/<name>) at those paths and creates the file. The call ends
by itself 1 s after the conversation, as TestCall hangs up (`--for N`: after N s; `--hold`: only at `end`).
Everything after the sources is Echo's real path: the watcher, the arm, the two tracks, live notes, the stop, the
transcription. What this can't test (the real tap of another app, the real mic device, #140) is the live check with
TestCall and BlackHole, run by a person (tests/app_profile.md, "The live check").

Standard library only.
"""
import json
import os
import shutil
import subprocess
import sys
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
MAKE_AUDIO = os.path.join(ROOT, "tests", "fixtures", "audio", "make_audio.py")


def die(msg):
    sys.exit(f"call: {msg}")


def env(name):
    v = os.environ.get(name) or die(f"{name} isn't set: `. <dir>/env` first (data from make_data.py)")
    return v


def paths():
    return env("ECHOCTL_TEST_ME_WAV"), env("ECHOCTL_TEST_REMOTE_WAV"), env("ECHOCTL_TEST_CALL_FILE")


def conversation(name):
    """The conversation's tracks, built once per run next to the data."""
    d = os.path.join(os.path.dirname(env("ECHOCTL_HOME")), "conv", name)
    if not os.path.exists(os.path.join(d, "expected.json")):
        r = subprocess.run([sys.executable, MAKE_AUDIO, "build", name, d], capture_output=True, text=True)
        if r.returncode != 0:
            die(f"can't build {name}: {r.stderr.strip() or r.stdout.strip()}")
    return d


def start(name, seconds, hold, meet=False, title=None, meet_app=False):
    me, remote, call = paths()
    if os.path.exists(call):
        die(f"a call is on already ({call}): `call.py end` first")
    conv = conversation(name)
    for src, dst in ((os.path.join(conv, "mic.wav"), me), (os.path.join(conv, "remote.wav"), remote)):
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        tmp = dst + ".tmp"
        shutil.copyfile(src, tmp)
        os.replace(tmp, dst)  # a recording that opens it now reads the whole file
    length = json.load(open(os.path.join(conv, "expected.json")))["duration"]
    open(call, "w").write(name + "\n" + ("app=meet\n" if meet else "") + (f"title={title}\n" if title else "")
                           + ("meet_app=1\n" if meet_app else ""))
    began = os.stat(call).st_mtime
    ends = None if hold else (seconds if seconds is not None else length + 1.0)
    if ends is not None:
        # Hang up after `ends` seconds, unless that call has ended already (its file gone or made again).
        script = (f"import os,time\ntime.sleep({ends})\n"
                  f"p={call!r}\n"
                  f"if os.path.exists(p) and os.stat(p).st_mtime == {began!r}: os.remove(p)\n")
        subprocess.Popen([sys.executable, "-c", script], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                         stderr=subprocess.DEVNULL, start_new_session=True)
    print(json.dumps({"conversation": name, "seconds": length, "started_at": time.strftime("%H:%M:%S", time.localtime(began)),
                      "hangs_up_in": ends, "call_file": call, "me": me, "remote": remote, "app": "meet" if meet else "teams"}))


def end():
    call = paths()[2]
    if os.path.exists(call):
        os.remove(call)
        print("hung up")
    else:
        print("no call")


def status():
    call = paths()[2]
    if os.path.exists(call):
        began = os.stat(call).st_mtime
        print(json.dumps({"in_call": True, "conversation": open(call).read().splitlines()[0], "for_s": round(time.time() - began, 1)}))
    else:
        print(json.dumps({"in_call": False}))


def main(argv):
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "start" and len(argv) > 2:
        seconds = float(argv[argv.index("--for") + 1]) if "--for" in argv else None
        title = argv[argv.index("--title") + 1] if "--title" in argv else None
        start(argv[2], seconds, "--hold" in argv, "--meet" in argv, title, "--meet-app" in argv)
    elif cmd == "end":
        end()
    elif cmd == "status":
        status()
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
