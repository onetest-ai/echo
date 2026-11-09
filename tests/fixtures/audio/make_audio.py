#!/usr/bin/env python3
"""Made-up calls with known words, as Echo records them: `mic.wav` (the user) and `remote.wav` (everyone else).

  make_audio.py render [--secrets FILE]       synthesise every turn of conversations.json not yet in clips/
  make_audio.py build <id> <call dir> [--languages en,uk,ru]
                                              a call folder from the clips: mic.wav, remote.wav (16 kHz mono), meta.json
                                              (its `languages`: the given ones, else the turns'), meeting.json and
                                              expected.json (each turn's who, text, lang, start, end)
  make_audio.py score <call dir> [--max-wer 0.15]
                                              the transcript (segments.json) against expected.json: word error rate
                                              per turn and overall, each segment's speaker and language, and nothing
                                              transcribed in silence; JSON, exit 1 when a check fails
  make_audio.py list                          the conversations

English goes to Azure OpenAI (gpt-4o-mini-tts), Ukrainian and Russian to ElevenLabs (eleven_multilingual_v2). Keys from
the environment (AZURE_TTS_ENDPOINT, AZURE_TTS_API_KEY, AZURE_TTS_DEPLOYMENT, ELEVENLABS_API_KEY) or from the `env` of a
JSON file given with --secrets; never written anywhere. Clips are cached as small mp3 files named by a hash of voice and
text (clips/, committed), so building needs no key and no network; only `render` does. Needs ffmpeg.
"""
import hashlib
import json
import os
import subprocess
import sys
import urllib.error
import urllib.request
import wave
from array import array
from datetime import datetime, timedelta
from pathlib import Path

HERE = Path(__file__).resolve().parent
CONV = json.loads((HERE / "conversations.json").read_text(encoding="utf-8"))
CLIPS = HERE / "clips"
RATE = 16000


def clip_path(voice: str, lang: str, text: str) -> Path:
    return CLIPS / (hashlib.sha1(f"{voice}|{lang}|{text}".encode()).hexdigest()[:16] + ".mp3")


def secrets(argv) -> dict:
    env = dict(os.environ)
    if "--secrets" in argv:
        env.update(json.loads(Path(argv[argv.index("--secrets") + 1]).read_text()).get("env", {}))
    return env


def post(url: str, headers: dict, body: dict) -> bytes:
    req = urllib.request.Request(url, data=json.dumps(body).encode(), headers={**headers, "Content-Type": "application/json"}, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=120) as r:
            return r.read()
    except urllib.error.HTTPError as e:
        sys.exit(f"{url.split('/')[2]} {e.code}: {e.read().decode(errors='replace')[:300]}")


def synth(env: dict, voice: str, lang: str, text: str) -> bytes:
    kind, name = voice.split(":", 1)
    if kind == "azure":
        ep = env["AZURE_TTS_ENDPOINT"].rstrip("/").split("/openai")[0]
        return post(f"{ep}/openai/v1/audio/speech", {"api-key": env["AZURE_TTS_API_KEY"]},
                    {"model": env["AZURE_TTS_DEPLOYMENT"], "input": text, "voice": name, "response_format": "mp3"})
    if kind == "eleven":
        return post(f"https://api.elevenlabs.io/v1/text-to-speech/{name}?output_format=mp3_22050_32", {"xi-api-key": env["ELEVENLABS_API_KEY"], "Accept": "audio/mpeg"},
                    {"text": text, "model_id": "eleven_multilingual_v2", "language_code": lang})
    sys.exit(f"unknown voice kind {voice}")


def render(argv):
    env = secrets(argv)
    CLIPS.mkdir(exist_ok=True)
    done = 0
    for c in CONV["conversations"]:
        for t in c["turns"]:
            voice = CONV["voices"][t["voice"]]
            p = clip_path(voice, t["lang"], t["text"])
            if p.exists():
                continue
            raw = p.with_suffix(".raw.mp3")
            raw.write_bytes(synth(env, voice, t["lang"], t["text"]))
            # Kept small: 16 kHz mono 32 kbit/s is what Echo transcribes from anyway.
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(raw), "-ac", "1", "-ar", str(RATE), "-b:a", "32k", str(p)], check=True)
            raw.unlink()
            done += 1
            print(f"rendered {c['id']}: {t['text'][:50]}")
    print(f"{done} new clip(s); {len(list(CLIPS.glob('*.mp3')))} in clips/")


def pcm(path: Path) -> array:
    raw = subprocess.run(["ffmpeg", "-v", "error", "-i", str(path), "-f", "s16le", "-ac", "1", "-ar", str(RATE), "-"], capture_output=True, check=True).stdout
    a = array("h")
    a.frombytes(raw)
    return a


def write_wav(path: Path, samples: array):
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(samples.tobytes())


def build(cid: str, out: Path, languages: list = None):
    conv = next((c for c in CONV["conversations"] if c["id"] == cid), None) or sys.exit(f"no conversation {cid}")
    mic, remote, turns, t = array("h"), array("h"), [], 0.0
    for turn in conv["turns"]:
        voice = CONV["voices"][turn["voice"]]
        p = clip_path(voice, turn["lang"], turn["text"])
        if not p.exists():
            sys.exit(f"missing clip for {turn['text'][:40]!r}: run `make_audio.py render` first")
        t += turn["gap"]
        s = pcm(p)
        start = int(t * RATE)
        for track in (mic, remote):
            track.extend([0] * max(0, start - len(track)))
        (mic if turn["who"] == "me" else remote).extend(s)
        end = t + len(s) / RATE
        turns.append({"who": turn["who"], "speaker": "me" if turn["who"] == "me" else "remote", "lang": turn["lang"], "text": turn["text"], "start": round(t, 2), "end": round(end, 2)})
        t = end
    n = max(len(mic), len(remote)) + RATE  # one second of silence after the last word
    for track in (mic, remote):
        track.extend([0] * (n - len(track)))
    out.mkdir(parents=True, exist_ok=True)
    write_wav(out / "mic.wav", mic)
    write_wav(out / "remote.wav", remote)
    begun = datetime.now().astimezone() - timedelta(seconds=n / RATE + 60)
    meta = {"app": "teams", "platform": "macos", "title": conv["title"], "languages": languages or sorted({x["lang"] for x in turns}),
            "started_at": begun.isoformat(timespec="seconds"), "ended_at": (begun + timedelta(seconds=n / RATE)).isoformat(timespec="seconds"),
            "stop_reason": "call_ended", "tracks": [{"file": "remote.wav", "sample_rate": RATE, "frames": n, "padded_frames": 0, "events": []},
                                                    {"file": "mic.wav", "sample_rate": RATE, "frames": n, "padded_frames": 0, "events": []}]}
    (out / "meta.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")
    (out / "expected.json").write_text(json.dumps({"id": cid, "what": conv["what"], "duration": round(n / RATE, 2), "turns": turns}, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"{out}: {n / RATE:.1f} s, {len(turns)} turns")


NUMBERS = {"0": "zero", "1": "one", "2": "two", "3": "three", "4": "four", "5": "five", "6": "six", "7": "seven", "8": "eight", "9": "nine",
           "10": "ten", "20": "twenty", "30": "thirty", "90": "ninety"}


def words(text: str) -> list:
    """Lower-case words without punctuation or quotes; English digits as words (so "4" matches "four")."""
    import re
    t = re.sub(r"[^\w\s'ʼ-]", " ", text.lower()).replace("-", " ")
    return [NUMBERS.get(w, w) for w in t.split()]


def wer(ref: list, hyp: list) -> float:
    d = list(range(len(hyp) + 1))
    for i, r in enumerate(ref, 1):
        prev, d[0] = d[0], i
        for j, h in enumerate(hyp, 1):
            prev, d[j] = d[j], min(d[j] + 1, d[j - 1] + 1, prev + (r != h))
    return d[len(hyp)] / max(len(ref), 1)


def score(call: Path, max_wer: float) -> int:
    exp = json.loads((call / "expected.json").read_text(encoding="utf-8"))
    segs = json.loads((call / "segments.json").read_text(encoding="utf-8"))
    turns = [{**t, "got": [], "speakers": set(), "langs": set()} for t in exp["turns"]]
    stray = []
    for s in segs:
        overlap = [(min(s["end"], t["end"]) - max(s["start"], t["start"]), t) for t in turns]
        best, t = max(overlap, key=lambda x: x[0]) if overlap else (0, None)
        if t is None or best <= 0:
            stray.append({"start": s["start"], "text": s["text"].strip()})
            continue
        t["got"].append(s["text"].strip())
        t["speakers"].add(s.get("speaker"))
        t["langs"].add(s.get("lang"))
    rows, errs, total = [], 0, 0
    for t in turns:
        ref, hyp = words(t["text"]), words(" ".join(t["got"]))
        w = wer(ref, hyp)
        errs += w * len(ref)
        total += len(ref)
        speaker_ok = t["speakers"] == {t["speaker"]}
        lang_ok = t["langs"] <= {t["lang"]} and bool(t["langs"])
        rows.append({"start": t["start"], "speaker": t["speaker"], "lang": t["lang"], "wer": round(w, 3), "speaker_ok": speaker_ok, "lang_ok": lang_ok,
                     "expected": t["text"], "got": " ".join(t["got"])})
    overall = errs / max(total, 1)
    ok = overall <= max_wer and all(r["speaker_ok"] and r["lang_ok"] for r in rows) and not stray
    print(json.dumps({"call": exp["id"], "pass": ok, "wer": round(overall, 3), "max_wer": max_wer, "turns": rows, "transcribed_in_silence": stray}, indent=1, ensure_ascii=False))
    return 0 if ok else 1


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "list"
    if cmd == "render":
        render(argv)
    elif cmd == "build" and len(argv) > 3:
        build(argv[2], Path(argv[3]), argv[argv.index("--languages") + 1].split(",") if "--languages" in argv else None)
    elif cmd == "score" and len(argv) > 2:
        sys.exit(score(Path(argv[2]), float(argv[argv.index("--max-wer") + 1]) if "--max-wer" in argv else 0.15))
    elif cmd == "list":
        for c in CONV["conversations"]:
            print(f"{c['id']:16} {c['title']:24} {len(c['turns'])} turns · {c['what']}")
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main(sys.argv)
