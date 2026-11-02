#!/usr/bin/env python3
"""Third-party notices for the components compiled into echoctl.

    scripts/third-party.py                 # rewrite THIRD-PARTY-NOTICES.md (the list, in the repo)
    scripts/third-party.py --texts <file>  # also write every crate's license files (for the app bundle)

Reads `cargo metadata` for the macOS and Windows targets; workspace crates are skipped.
"""
import glob, json, os, re, subprocess, sys, urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TARGETS = ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]
# Not crates, but shipped or used: the C++ engine inside whisper-rs-sys and the downloaded models.
EXTRA = [
    ("whisper.cpp", "(bundled in whisper-rs-sys)", "MIT", "https://github.com/ggml-org/whisper.cpp"),
    ("Whisper models (downloaded at setup)", "", "MIT", "https://github.com/openai/whisper"),
    ("Vulkan loader (vulkan-1.dll, Windows zip)", "", "Apache-2.0", "https://github.com/KhronosGroup/Vulkan-Loader"),
    ("NeMo-Speech.cpp runtime (Nemotron models; its components' licenses ship in nemo-speech/licenses)", "0.2.0", "Apache-2.0", "https://github.com/NVIDIA/NeMo-Speech.cpp"),
    ("Nemotron speech models (downloaded when chosen)", "", "OpenMDW-1.1", "https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b"),
    ("mermaid (diagrams in the Echo window; license in ui/vendor/mermaid.min.js)", "11.17.2", "MIT", "https://github.com/mermaid-js/mermaid"),
    ("xterm.js (terminals in the Echo window; license in ui/vendor/xterm.LICENSE)", "5.5.0", "MIT", "https://github.com/xtermjs/xterm.js"),
    ("@xterm/addon-fit (terminals in the Echo window)", "0.10.0", "MIT", "https://github.com/xtermjs/xterm.js"),
]
# Vendored files whose license text isn't inside them: their texts go into the app's licenses file.
VENDORED_TEXTS = [
    ("xterm.js 5.5.0 and @xterm/addon-fit 0.10.0 (MIT)", "https://github.com/xtermjs/xterm.js", "crates/echoctl/ui/vendor/xterm.LICENSE"),
]


def packages():
    seen = {}
    for t in TARGETS:
        out = subprocess.run(["cargo", "metadata", "--format-version", "1", "--filter-platform", t],
                             cwd=ROOT, check=True, capture_output=True, text=True, encoding="utf-8").stdout
        m = json.loads(out)
        nodes = {n["id"] for n in m["resolve"]["nodes"]}
        for p in m["packages"]:
            if p["id"] in m["workspace_members"] or p["id"] not in nodes or p["source"] is None:
                continue
            seen[(p["name"], p["version"])] = p
    return [seen[k] for k in sorted(seen)]


def license_files(p):
    d = os.path.dirname(p["manifest_path"])
    files = [f for pat in ("LICEN[CS]E*", "COPYING*", "NOTICE*", "UNLICENSE*") for f in glob.glob(os.path.join(d, pat))]
    if p.get("license_file"):
        files.append(os.path.join(d, p["license_file"]))
    return sorted({f for f in files if os.path.isfile(f)})


def main():
    pkgs = packages()
    rows = [(p["name"], p["version"], p.get("license") or "see crate", p.get("repository") or "") for p in pkgs]
    with open(os.path.join(ROOT, "THIRD-PARTY-NOTICES.md"), "w", encoding="utf-8", newline="\n") as f:
        f.write("# Third-party notices\n\n"
                "Echo is built on the open-source components below. Each remains under its own license; "
                "the full license texts ship inside the app (`Echo.app/Contents/Resources/THIRD-PARTY-LICENSES.txt`).\n"
                "Regenerate with `scripts/third-party.py`.\n\n"
                "| Component | Version | License | Source |\n|---|---|---|---|\n")
        for name, ver, lic, repo in [*EXTRA, *rows]:
            f.write(f"| {name} | {ver} | {lic} | {repo} |\n")
    if "--texts" in sys.argv:
        out = sys.argv[sys.argv.index("--texts") + 1]
        missing = set()
        with open(out, "w", encoding="utf-8") as f:
            f.write("Licenses of the third-party components in Echo.\n\n")
            for p in pkgs:
                f.write(f"{'=' * 78}\n{p['name']} {p['version']}  ({p.get('license') or 'see below'})\n"
                        f"{p.get('repository') or ''}\n{'=' * 78}\n\n")
                texts = license_files(p)
                for t in texts:
                    f.write(open(t, encoding="utf-8", errors="replace").read().rstrip() + "\n\n")
                if not texts:
                    ids = re.findall(r"[A-Za-z0-9.-]+", (p.get("license") or "").replace("/", " OR "))
                    ids = [i for i in ids if i not in ("OR", "AND", "WITH")]
                    missing.update(ids)
                    f.write(f"Licensed under {p.get('license')}; the standard text is in the last section.\n\n")
            for name, repo, path in VENDORED_TEXTS:
                f.write(f"{'=' * 78}\n{name}\n{repo}\n{'=' * 78}\n\n")
                f.write(open(os.path.join(ROOT, path), encoding="utf-8").read().rstrip() + "\n\n")
            # Crates that publish no license file: the standard SPDX texts of their licenses.
            f.write(f"{'=' * 78}\nStandard license texts\n{'=' * 78}\n\n")
            for i in sorted(missing):
                url = f"https://raw.githubusercontent.com/spdx/license-list-data/main/text/{i}.txt"
                f.write(f"--- {i} ---\n\n{urllib.request.urlopen(url, timeout=30).read().decode().rstrip()}\n\n")
    print(f"{len(pkgs)} crates")


if __name__ == "__main__":
    main()
