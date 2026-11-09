#!/usr/bin/env python3
"""Recorded calls for functional tests: record and transcribe each known conversation ONCE per build, then copy.

    tests/fixtures/recordings.py prepare [--force]     record + transcribe what the cases need (once per build)
    tests/fixtures/recordings.py path                  the cache folder of this build (exit 1 if not prepared)
    tests/fixtures/recordings.py list                  what the cache holds, with each recording's score
    tests/fixtures/recordings.py copy <name> [--title T] [--untranscribed] [--keep-time]
                                                       a copy of a cached call into $ECHOCTL_HOME/calls; prints its folder
    tests/fixtures/recordings.py finish <folder> <name>
                                                       give a copy made with --untranscribed its transcript

Recording and transcribing is a prerequisite, not what most cases test: `prepare` feeds each conversation of
tests/fixtures/audio to `echoctl capture` from files (the debug build's test sources, crates/echoctl/src/audio/testfeed.rs:
no device opened, no sound played; the captures run side by side), records it (two tracks, the real capture path), and
transcribes it with the build's speech model, once. The call folders (meta.json, segments.json, transcript.md,
expected.json; the audio deleted as Echo deletes it) and what the commands printed are kept in
target/test-recordings/<key>/ (git-ignored), <key> the last commit that changed the app (crates/, Cargo.*), "-dirty"
when they have uncommitted changes. Every scenario then copies the calls it needs into its own fresh data folder: no
capture and no transcription of its own, unless capturing is what the case checks.

`copy` gives the copy a fresh stamp and makes it end now (start moved by the same amount), so it counts as a call that
just ended (automatic minutes, Today); --keep-time keeps the recorded times. --untranscribed copies meta.json only (a
call just recorded, not transcribed yet); `finish` then adds its transcript, as the transcription would.

Needs, for `prepare`: the debug build (`cargo build`; another with ECHOCTL_BIN), the speech model in
~/Library/Application Support/echo/models or the folder ECHOCTL_TEST_MODELS names (linked, never changed), and the
network once for NVIDIA's NeMo-Speech runtime (speakers). No TestCall, BlackHole or ffmpeg: it opens no device and
plays nothing. Standard library only.
"""
import json
import os
import re
import shutil
import subprocess
import sys
import time
import wave
from array import array
from datetime import datetime, timedelta

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
BIN = os.environ.get("ECHOCTL_BIN") or os.path.join(ROOT, "target", "debug", "echoctl")
MAKE_AUDIO = os.path.join(ROOT, "tests", "fixtures", "audio", "make_audio.py")
MODELS = os.environ.get("ECHOCTL_TEST_MODELS") or os.path.expanduser("~/Library/Application Support/echo/models")
MODEL = "large-v3-turbo-q5_0"

# What `prepare` makes. name → (conversation, how): "capture" records it with `echoctl capture` fed from the
# conversation's files for the call's length + 3.5 s; "build" writes the tracks straight
# from the clips (no capture: the minutes cases need a long call, not a recording); "retranscribe" transcribes the
# tracks of another recording again with other settings.
RECORDINGS = [
    {"name": "acme-sync", "conv": "acme-sync", "how": "capture", "languages": "en",
     "for": "words, speakers and order (transcription/TC-001), a capture's own record (recording/TC-014 step 4), no audio kept after `transcribe` (privacy/TC-003)"},
    {"name": "very-short", "conv": "very-short", "how": "capture", "languages": "en",
     "for": "a call too short for automatic minutes (recording/TC-013, claude/TC-004), the transcription server (transcription/TC-007)"},
    {"name": "long-silence", "conv": "long-silence", "how": "capture", "languages": "en",
     "for": "silence in a recorded call (recording/TC-017, transcription/TC-005)"},
    {"name": "two-remotes", "conv": "two-remotes", "how": "capture", "languages": "en", "speakers": True,
     "for": "two remote voices told apart (recording/TC-018), one language set (transcription/TC-003)"},
    {"name": "mixed-languages-pinned", "conv": "mixed-languages", "how": "capture", "languages": "en,uk,ru",
     "pin": "uk", "for": "a language pinned for one call (transcription/TC-004)"},
    {"name": "mixed-languages", "conv": "mixed-languages", "how": "retranscribe", "from": "mixed-languages-pinned",
     "languages": "uk,ru,en", "for": "the next call after a pinned one: the workspace languages, its one language detected (transcription/TC-004)"},
    {"name": "ukrainian-call", "conv": "ukrainian-call", "how": "capture", "languages": "en,uk,ru",
     "for": "a call in Ukrainian comes out in Ukrainian among en, uk and ru (transcription/TC-002)"},
    {"name": "russian-call", "conv": "russian-call", "how": "capture", "languages": "en,uk,ru",
     "for": "a call in Russian comes out in Russian among en, uk and ru (transcription/TC-002)"},
    {"name": "minutes-call", "conv": "minutes-call", "how": "build", "languages": "en",
     "for": "a call long enough for minutes (claude/TC-003, TC-004, TC-007, TC-009, TC-010, TC-015)"},
]


def die(msg):
    sys.exit(f"recordings: {msg}")


def git(*args):
    return subprocess.run(["git", "-C", ROOT, *args], capture_output=True, text=True).stdout.strip()


def key():
    """The last commit that changed the app, "-dirty" with uncommitted changes there: a test-only commit keeps the cache."""
    paths = ["crates", "Cargo.toml", "Cargo.lock"]
    k = git("log", "-1", "--format=%h", "--", *paths) or "unknown"
    return k + ("-dirty" if git("status", "--porcelain", "--", *paths) else "")


def cache():
    return os.path.join(ROOT, "target", "test-recordings", key())


def load_manifest(c):
    try:
        return json.load(open(os.path.join(c, "manifest.json")))
    except (OSError, ValueError):
        return None


def ready():
    c = cache()
    m = load_manifest(c)
    return c if m and m.get("complete") else None


# ── prepare ───────────────────────────────────────────────────────────────────────────────────
def peak(path):
    with wave.open(path) as w:
        a = array("h", w.readframes(w.getnframes()))
    return max((abs(x) for x in a), default=0)


def prepare(force):
    c = cache()
    if ready() and not force:
        print(c)
        return
    for need, what in ((BIN, "cargo build"), (os.path.join(MODELS, f"ggml-{MODEL}.bin"), "the speech model")):
        if not os.path.exists(need):
            die(f"missing {need} ({what})")
    shutil.rmtree(c, ignore_errors=True)
    os.makedirs(c)
    work = os.path.join(c, "work")
    home, config = os.path.join(work, "home"), os.path.join(work, "config")
    os.makedirs(os.path.join(home, "calls"))
    os.makedirs(config)
    os.symlink(MODELS, os.path.join(config, "models"))
    env = dict(os.environ, ECHOCTL_HOME=home, ECHOCTL_CONFIG_DIR=config, ECHOCTL_CALL_APP_BUNDLE="ai.onetest.echo.testcall")
    for k in ("CLAUDECODE", "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_CHILD_SESSION", "AI_AGENT", "ECHOCTL_TEST_AUTO_MINUTES_DELAY_S",
              "ECHOCTL_TEST_CALL_FILE", "ECHOCTL_TEST_ME_WAV", "ECHOCTL_TEST_REMOTE_WAV"):
        env.pop(k, None)
    log = open(os.path.join(c, "prepare.log"), "w")

    def echoctl(*args, out=None, env_=None):
        r = subprocess.run([BIN, *args], env=env_ or env, cwd=ROOT, capture_output=True, text=True)
        log.write(f"$ echoctl {' '.join(args)}  → {r.returncode}\n{r.stdout}{r.stderr}\n")
        log.flush()
        if out:
            open(out, "w").write(r.stdout + r.stderr)
        return r

    if echoctl("model", "use", MODEL).returncode != 0:
        die(f"`echoctl model use {MODEL}` failed: see {c}/prepare.log")
    manifest = {"key": key(), "build": BIN, "model": MODEL, "made_at": datetime.now().astimezone().isoformat(timespec="seconds"),
                "complete": False, "recordings": {}}

    def save():
        json.dump(manifest, open(os.path.join(c, "manifest.json"), "w"), indent=2, ensure_ascii=False)

    nemo = None
    # Every capture at once, each with its own config (its languages): fed from files, they share no device.
    running = {}
    for r in RECORDINGS:
        conv = os.path.join(c, "conv", r["conv"])
        if not os.path.isdir(conv):
            subprocess.run([sys.executable, MAKE_AUDIO, "build", r["conv"], conv], check=True, capture_output=True)
        if r["how"] == "capture":
            out = os.path.join(c, r["name"])
            os.makedirs(out)
            cfg = os.path.join(work, "config-" + r["name"])
            os.makedirs(cfg)
            os.symlink(MODELS, os.path.join(cfg, "models"))
            renv = dict(env, ECHOCTL_CONFIG_DIR=cfg)
            echoctl("config", "--languages", r["languages"], env_=renv)
            r["seconds"] = int(json.load(open(os.path.join(conv, "expected.json")))["duration"] + 3.5)
            running[r["name"]] = (capture(r, conv, out, renv), renv, time.monotonic())
    time.sleep(8)
    for r in RECORDINGS:
        if r["name"] in running:
            during(r, os.path.join(c, r["name"]), home)
    for r in RECORDINGS:
        name, out = r["name"], os.path.join(c, r["name"])
        os.makedirs(out, exist_ok=True)
        echoctl("config", "--languages", r["languages"])
        conv = os.path.join(c, "conv", r["conv"])
        entry = {"conversation": r["conv"], "how": r["how"], "for": r["for"], "languages": r["languages"]}
        t0 = time.monotonic()
        if r["how"] == "capture":
            started, renv, t0 = running[name]
            call = finish_capture(r, out, started, log)
            entry.update(seconds=r["seconds"], capture_exit=open(os.path.join(out, "capture.exit")).read().strip())
            if r.get("pin"):
                entry["pin"] = r["pin"]
                echoctl("config", out=os.path.join(out, "config.after.json"), env_=renv)
        elif r["how"] == "build":
            call = os.path.join(home, "calls", datetime.now().strftime("%Y-%m-%d_%H%M%S") + "-" + name)
            subprocess.run([sys.executable, MAKE_AUDIO, "build", r["conv"], call], check=True, capture_output=True)
        else:  # retranscribe: the same tracks, the call's own pin left out
            src = os.path.join(c, r["from"])
            call = os.path.join(home, "calls", datetime.now().strftime("%Y-%m-%d_%H%M%S") + "-" + name)
            os.makedirs(call)
            for f in ("mic.wav", "remote.wav"):
                shutil.copy(os.path.join(src, "tracks", f), call)
            meta = json.load(open(os.path.join(src, "meta.json")))
            meta.pop("languages", None)
            meta.pop("language", None)  # one a capture's transcription decided
            meta["title"] = name
            json.dump(meta, open(os.path.join(call, "meta.json"), "w"), indent=2)
        entry["record_s"] = round(time.monotonic() - t0, 1)
        os.makedirs(os.path.join(out, "tracks"), exist_ok=True)
        for f in ("mic.wav", "remote.wav"):
            p = os.path.join(call, f)
            if not os.path.exists(p):
                die(f"{name}: no {f} in {call}: see {out}/capture.err")
            shutil.copy(p, os.path.join(out, "tracks", f))
        if r["how"] == "capture":
            loud = max(peak(os.path.join(call, f)) for f in ("mic.wav", "remote.wav"))
            if loud < 64:
                die(f"{name}: the tracks are silent (peak {loud}): see {out}/capture.err (the test sources are a debug build's)")
        shutil.copy(os.path.join(conv, "expected.json"), call)
        if r.get("speakers"):
            nemo = nemo or fetch_nemo(c, log)
            os.symlink(nemo, os.path.join(config, "nemo-speech"))
        t0 = time.monotonic()
        tr = echoctl("transcribe", call, out=os.path.join(out, "transcribe.out"))
        entry["transcribe_s"] = round(time.monotonic() - t0, 1)
        if r.get("speakers"):
            os.unlink(os.path.join(config, "nemo-speech"))
            echoctl("speakers", call, out=os.path.join(out, "speakers.out"))
        if tr.returncode != 0:
            die(f"{name}: transcribe failed: see {out}/transcribe.out")
        open(os.path.join(out, "after.ls"), "w").write("\n".join(sorted(os.listdir(call))) + "\n")
        s = subprocess.run([sys.executable, MAKE_AUDIO, "score", call], capture_output=True, text=True)
        open(os.path.join(out, "score.json"), "w").write(s.stdout)
        try:
            sc = json.loads(s.stdout)
            entry.update(score_pass=sc["pass"], wer=sc["wer"], in_silence=len(sc["transcribed_in_silence"]))
        except ValueError:
            entry.update(score_pass=None)
        os.makedirs(os.path.join(out, "call"))
        shutil.move(call, os.path.join(out, "call", os.path.basename(call)))
        entry["folder"] = os.path.basename(call)
        manifest["recordings"][name] = entry
        save()
        print(f"{name:24} {entry['how']:13} record {entry['record_s']:6.1f} s  transcribe {entry['transcribe_s']:5.1f} s  "
              f"pass {entry.get('score_pass')}  wer {entry.get('wer')}", file=sys.stderr)
    manifest["complete"] = True
    save()
    shutil.rmtree(os.path.join(c, "work", "home"), ignore_errors=True)
    print(c)


def capture(r, conv, out, env):
    """Start one `echoctl capture` fed from the conversation's files (no device, no sound); `finish_capture` waits."""
    env = dict(env, ECHOCTL_TEST_ME_WAV=os.path.join(conv, "mic.wav"), ECHOCTL_TEST_REMOTE_WAV=os.path.join(conv, "remote.wav"))
    args = [BIN, "capture", "--seconds", str(r["seconds"]), "--title", r["name"]] + (["--languages", r["pin"]] if r.get("pin") else [])
    so, se = open(os.path.join(out, "capture.out"), "w"), open(os.path.join(out, "capture.err"), "w")
    return subprocess.Popen(args, env=env, cwd=ROOT, stdout=so, stderr=se), args, env["ECHOCTL_HOME"]


def during(r, out, home):
    """While it records: the files of its call folder (privacy/TC-003 reads this)."""
    calls = os.path.join(home, "calls")
    rec = [d for d in os.listdir(calls) if d.endswith("-" + r["name"])]
    open(os.path.join(out, "during.ls"), "w").write("".join("\n".join(sorted(os.listdir(os.path.join(calls, d)))) + "\n" for d in rec))


def finish_capture(r, out, started, log):
    """Wait for a capture `capture` started; keep what it printed and the files while it recorded."""
    cap, args, _ = started
    code = cap.wait()
    open(os.path.join(out, "capture.exit"), "w").write(f"{code}\n")
    log.write(f"$ {' '.join(args)}  → {code} (134 after saving is #141)\n")
    lines = [ln.strip() for ln in open(os.path.join(out, "capture.out")) if ln.strip()]
    if not lines or not os.path.isdir(lines[-1]):
        die(f"{r['name']}: capture printed no call folder (exit {code}): see {out}/capture.err")
    shutil.copy(os.path.join(lines[-1], "meta.json"), os.path.join(out, "meta.json"))
    return lines[-1]


def fetch_nemo(c, log):
    """NVIDIA's NeMo-Speech runtime (speaker detection), fetched once for every build's cache."""
    dest = os.path.join(ROOT, "target", "test-recordings", "nemo-speech")
    if not os.path.exists(os.path.join(dest, "libnemo_speech_asr_c.1.dylib")):
        tmp = os.path.join(ROOT, "target", "test-recordings", "tmp")
        os.makedirs(tmp, exist_ok=True)
        r = subprocess.run([os.path.join(ROOT, "scripts", "fetch-nemo-speech.sh"), dest], env=dict(os.environ, TMPDIR=tmp), capture_output=True, text=True)
        log.write(r.stdout + r.stderr)
        if r.returncode != 0:
            die(f"fetching the NeMo-Speech runtime failed: {r.stderr.strip()}")
    return dest


# ── copy ──────────────────────────────────────────────────────────────────────────────────────
def slug(title):
    return re.sub(r"[^a-z0-9]+", "-", title.lower()).strip("-")


def cached_call(name):
    c = ready() or die("nothing prepared for this build: run `tests/fixtures/recordings.py prepare` first")
    m = load_manifest(c)["recordings"].get(name) or die(f"no recording {name!r} (list: {', '.join(r['name'] for r in RECORDINGS)})")
    return os.path.join(c, name, "call", m["folder"])


def rewrite_transcript(src, dst, title, folder, started):
    """transcript.md with the copy's title, start and folder; the rest as transcribed."""
    text = open(src, encoding="utf-8").read()
    text = re.sub(r"\A# .*", lambda _: f"# {title}", text, count=1)
    if started:
        text = re.sub(r"(\*\*Recorded:\*\* ).*", lambda m: m.group(1) + started, text, count=1)
    text = re.sub(r"(\*\*Folder:\*\* `)[^`]*(`)", lambda m: m.group(1) + folder + m.group(2), text)
    open(dst, "w", encoding="utf-8").write(text)


def copy(name, title, untranscribed, keep_time):
    home = os.environ.get("ECHOCTL_HOME") or die("ECHOCTL_HOME isn't set: `. <dir>/env` first")
    if os.path.realpath(home).startswith(os.path.realpath(os.path.expanduser("~/echo"))):
        die("refusing the real data folder")
    src = cached_call(name)
    meta = json.load(open(os.path.join(src, "meta.json")))
    title = title or meta.get("title") or name
    start, end = (datetime.fromisoformat(meta[k]) if meta.get(k) else None for k in ("started_at", "ended_at"))
    now = datetime.now().astimezone()
    if not keep_time and start and end:
        start, end = now - (end - start), now
        meta["started_at"], meta["ended_at"] = start.isoformat(), end.isoformat()
    meta["title"] = title
    calls = os.path.join(home, "calls")
    os.makedirs(calls, exist_ok=True)
    stamp = start or now
    while True:  # a stamp of its own (two copies in one second)
        folder = f"{stamp.strftime('%Y-%m-%d_%H%M%S')}-{slug(title)}"
        if not any(d.startswith(folder[:17]) for d in os.listdir(calls)):
            break
        stamp += timedelta(seconds=1)
    dst = os.path.join(calls, folder)
    os.makedirs(dst)
    json.dump(meta, open(os.path.join(dst, "meta.json"), "w"), indent=2, ensure_ascii=False)
    shutil.copy(os.path.join(src, "expected.json"), dst)
    if not untranscribed:
        finish(dst, name)
    print(dst)


def finish(folder, name):
    folder = folder if os.path.isabs(folder) else os.path.join(os.environ.get("ECHOCTL_HOME", ""), "calls", folder)
    if not os.path.isdir(folder):
        die(f"no folder {folder}")
    src = cached_call(name)
    meta = json.load(open(os.path.join(folder, "meta.json")))
    title = meta.get("title") or name
    for f in sorted(os.listdir(src)):
        if f in ("meta.json", "expected.json", "transcript.md", ".meta.lock") or f.endswith(".wav"):
            continue
        p = os.path.join(src, f)
        (shutil.copytree if os.path.isdir(p) else shutil.copy)(p, os.path.join(folder, f))
    rewrite_transcript(os.path.join(src, "transcript.md"), os.path.join(folder, "transcript.md"), title, os.path.basename(folder),
                       meta.get("started_at"))


def show():
    c = ready() or die("nothing prepared for this build: run `tests/fixtures/recordings.py prepare` first")
    for name, e in load_manifest(c)["recordings"].items():
        print(f"{name:24} {e['how']:13} {e['folder']:48} pass {e.get('score_pass')}  wer {e.get('wer')}  — {e['for']}")


def main(argv):
    cmd = argv[1] if len(argv) > 1 else ""
    if cmd == "prepare":
        prepare("--force" in argv)
    elif cmd == "path":
        print(ready() or die("nothing prepared for this build"))
    elif cmd == "list":
        show()
    elif cmd == "copy" and len(argv) > 2:
        title = argv[argv.index("--title") + 1] if "--title" in argv else None
        copy(argv[2], title, "--untranscribed" in argv, "--keep-time" in argv)
    elif cmd == "finish" and len(argv) > 3:
        finish(argv[2], argv[3])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
