#!/usr/bin/env python3
"""Echo's functional map: tests/feature-map/ (JSON for agents) → checks and an HTML page (for people).

The map is split so features of different parts of the app can be worked on in parallel without merge conflicts:
  areas/<prefix>.json   one area: {prefix, name, order, status, notes, features: [{id, feature, priority, check, axes}]}
  axes/<entity>.json    one entity's state axes: {entity, axes: [{axis, values, where}]}
  combinations.json     axes to cover pairwise, and why
  hostile.json          where others' text reaches the window
  about.md              what the map is and how to keep it

  tests/tools/feature-map.py check            ids, areas and axes consistent; every case's requirements known;
                                              every agent case in a scenario (exit 1 on a problem)
  tests/tools/feature-map.py render [OUT]     the map as one HTML page with coverage (default
                                              tests/reports/feature-map.html)

Coverage: a case lists the features it checks in its frontmatter (`requirements: [F-CALLS-01, …]`); a scenario lists
its cases (`cases:`). A feature is covered by agent cases, manual cases, or none.
"""
import html
import json
import re
import sys
from pathlib import Path

TESTS = Path(__file__).resolve().parent.parent
MAP = TESTS / "feature-map"
PRIORITIES = ["critical", "high", "medium", "low"]


def frontmatter(path):
    """The YAML-ish frontmatter of a case or scenario: scalars, inline lists and `- item` lists."""
    text = path.read_text(encoding="utf-8")
    m = re.match(r"---\n(.*?)\n---", text, re.S)
    out, key = {}, None
    for line in (m.group(1) if m else "").splitlines():
        if re.match(r"\s+- ", line) and key:
            out.setdefault(key, []).append(line.strip()[2:].strip())
            continue
        k, _, v = line.partition(":")
        key, v = k.strip(), v.split("#")[0].strip()
        if v.startswith("[") and v.endswith("]"):
            out[key] = [x.strip() for x in v[1:-1].split(",") if x.strip()]
        elif v:
            out[key] = v
    return out


def load_map():
    """The map's files as one document; each feature carries its area's prefix and its file."""
    read = lambda p: json.loads(p.read_text(encoding="utf-8"))
    areas, features, axes = [], [], []
    for p in sorted((MAP / "areas").glob("*.json")):
        a = read(p)
        areas.append({k: a.get(k, "") for k in ("prefix", "name", "order", "status", "notes")})
        features += [{**f, "area": a["prefix"], "file": f"areas/{p.name}"} for f in a["features"]]
    areas.sort(key=lambda a: (a["order"] or 0, a["name"]))
    order = {a["prefix"]: i for i, a in enumerate(areas)}
    features.sort(key=lambda f: order[f["area"]])
    for p in sorted((MAP / "axes").glob("*.json")):
        e = read(p)
        axes += [{**x, "entity": e["entity"], "file": f"axes/{p.name}"} for x in e["axes"]]
    return {"title": "Echo — functional map", "about": (MAP / "about.md").read_text(encoding="utf-8").split("\n", 1)[1].strip(),
            "areas": areas, "features": features, "axes": axes, "combinations": read(MAP / "combinations.json"), "hostile": read(MAP / "hostile.json")}


def load():
    doc = load_map()
    cases = {}
    for p in sorted((TESTS / "cases").glob("*/TC-*.md")):
        fm = frontmatter(p)
        cases[str(p.relative_to(TESTS.parent))] = {"area": p.parent.name, "id": fm.get("id", ""), "title": fm.get("title", ""),
                                                   "run": fm.get("run", ""), "requirements": fm.get("requirements", []) or []}
    scenarios = {}
    for p in sorted((TESTS / "scenarios").glob("S-*.md")):
        fm = frontmatter(p)
        scenarios[str(p.relative_to(TESTS.parent))] = {"id": fm.get("id", ""), "title": fm.get("title", ""), "cases": fm.get("cases", []) or []}
    return doc, cases, scenarios


def problems(doc, cases, scenarios):
    out = []
    prefixes = {a["prefix"] for a in doc["areas"]}
    axes = {a["axis"] for a in doc["axes"]}
    seen = set()
    if len(prefixes) != len(doc["areas"]):
        out.append("two area files share a prefix")
    ax_seen = set()
    for a in doc["axes"]:
        if a["axis"] in ax_seen:
            out.append(f"{a['file']}: axis {a['axis']} defined twice")
        ax_seen.add(a["axis"])
    for f in doc["features"]:
        if f["id"] in seen:
            out.append(f"{f['file']}: {f['id']} used twice")
        seen.add(f["id"])
        if f["area"] not in prefixes:
            out.append(f"{f['id']}: unknown area {f['area']}")
        if not f["id"].startswith(f"F-{f['area']}-"):
            out.append(f"{f['id']}: id doesn't match its area {f['area']}")
        if f["priority"] not in PRIORITIES or f["check"] not in ("agent", "manual"):
            out.append(f"{f['id']}: priority/check must be one of {PRIORITIES} / agent, manual")
        out += [f"{f['id']}: unknown axis {x}" for x in f["axes"] if x not in axes]
    for c in doc["combinations"]:
        out += [f"combination: unknown axis {x}" for x in c["axes"] if x not in axes]
    for path, c in cases.items():
        out += [f"{path}: unknown requirement {r}" for r in c["requirements"] if r not in seen]
    in_scenario = {c for s in scenarios.values() for c in s["cases"]}
    for path, s in scenarios.items():
        out += [f"{path}: no such case {c}" for c in s["cases"] if c not in cases]
    out += [f"{path}: run: agent case in no scenario" for path, c in cases.items() if c["run"] == "agent" and path not in in_scenario]
    return out


def coverage(doc, cases):
    by = {f["id"]: {"agent": [], "manual": []} for f in doc["features"]}
    for path, c in cases.items():
        for r in c["requirements"]:
            if r in by and c["run"] in ("agent", "manual"):
                by[r][c["run"]].append(path)
    return by


def render(doc, cases, scenarios, out):
    e = html.escape
    cov = coverage(doc, cases)
    feats = doc["features"]
    def state(f):
        c = cov[f["id"]]
        return "agent" if c["agent"] else "manual" if c["manual"] else "none"
    n = len(feats)
    covered = sum(state(f) != "none" for f in feats)
    agent_ok = sum(bool(cov[f["id"]]["agent"]) for f in feats if f["check"] == "agent")
    agent_n = sum(f["check"] == "agent" for f in feats)
    crit = [f for f in feats if f["priority"] == "critical"]
    crit_ok = sum(state(f) != "none" for f in crit)
    case_of = {p: c for p, c in cases.items()}

    rows = []
    for a in doc["areas"]:
        fs = [f for f in feats if f["area"] == a["prefix"]]
        if not fs:
            continue
        ok = sum(state(f) != "none" for f in fs)
        rows.append(f'<tr class="area-head" data-area="{e(a["prefix"])}"><th colspan="6">{e(a["name"])} <span class="muted">F-{e(a["prefix"])} · {e(a["status"])} · '
                    f'{ok}/{len(fs)} covered</span><div class="muted small">{e(a.get("notes", ""))}</div></th></tr>')
        for f in fs:
            c = cov[f["id"]]
            links = "".join(f'<a class="case {k}" href="../../{e(p)}" title="{e(case_of[p]["title"])}">{e(case_of[p]["area"])}/{e(case_of[p]["id"])}</a>'
                            for k in ("agent", "manual") for p in c[k])
            rows.append(f'<tr data-area="{e(f["area"])}" data-prio="{e(f["priority"])}" data-check="{e(f["check"])}" data-cov="{state(f)}">'
                        f'<td class="mono" style="white-space:nowrap">{e(f["id"])}</td><td>{e(f["feature"])}{" <b class=review>review</b>" if f.get("review") else ""}</td>'
                        f'<td><span class="prio {e(f["priority"])}">{e(f["priority"])}</span></td><td>{e(f["check"])}</td>'
                        f'<td class="mono small">{"<br>".join(e(x) for x in f["axes"])}</td><td>{links or "<span class=gap>none</span>"}</td></tr>')
    axes_html, ent = [], None
    for a in doc["axes"]:
        if a["entity"] != ent:
            ent = a["entity"]
            axes_html.append(f'<tr class="area-head"><th colspan="3">{e(ent)}</th></tr>')
        axes_html.append(f'<tr><td class="mono">{e(a["axis"])}</td><td>{e(a["values"])}</td><td class="small muted">{e(a["where"])}</td></tr>')
    combos = "".join(f'<tr><td class="mono small">{" × ".join(e(x) for x in c["axes"])}</td><td>{e(c["why"])}</td></tr>' for c in doc["combinations"])
    scen = "".join(f'<li><a href="../../{e(p)}">{e(s["id"])}</a> {e(s["title"])} <span class="muted">({len(s["cases"])} stages)</span></li>' for p, s in scenarios.items())
    hostile = "".join(f"<li>{e(x)}</li>" for x in doc["hostile"]["places"])
    opts = lambda vals: "".join(f'<option value="{e(v)}">{e(l)}</option>' for v, l in vals)
    page = f"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>{e(doc["title"])}</title>
<meta name="viewport" content="width=device-width,initial-scale=1"><style>
:root{{--bg:#fbfaf8;--fg:#1d1b20;--mut:#6b6670;--line:#e6e2ea;--acc:#7c5cff;--ok:#2f8f5b;--warn:#c47a12;--bad:#c2410c}}
@media (prefers-color-scheme:dark){{:root{{--bg:#18171b;--fg:#ece9f1;--mut:#9a94a3;--line:#2c2a31;--acc:#a48bff}}}}
body{{margin:0;font:14px/1.45 -apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;background:var(--bg);color:var(--fg)}}
main{{max-width:1280px;margin:0 auto;padding:28px 24px 80px}} h1{{margin:0 0 4px;font-size:24px}} h2{{margin:36px 0 10px;font-size:17px}}
.muted{{color:var(--mut);font-weight:400}} .small{{font-size:12px}} .mono{{font-family:ui-monospace,Menlo,monospace;font-size:12px}}
.stats{{display:flex;gap:12px;flex-wrap:wrap;margin:18px 0}} .stat{{border:1px solid var(--line);border-radius:10px;padding:10px 14px;min-width:150px}}
.stat b{{display:block;font-size:22px}} .bar{{height:6px;background:var(--line);border-radius:3px;margin-top:6px}} .bar i{{display:block;height:6px;border-radius:3px;background:var(--acc)}}
.filters{{display:flex;gap:8px;flex-wrap:wrap;align-items:center;position:sticky;top:0;background:var(--bg);padding:10px 0;z-index:2;border-bottom:1px solid var(--line)}}
select,input{{font:inherit;padding:5px 8px;border:1px solid var(--line);border-radius:7px;background:var(--bg);color:var(--fg)}} input{{min-width:240px}}
table{{width:100%;border-collapse:collapse}} td,th{{text-align:left;vertical-align:top;padding:7px 8px;border-bottom:1px solid var(--line)}}
tr.area-head th{{padding-top:20px;font-size:15px;border-bottom:2px solid var(--line)}}
.prio{{font-size:11px;padding:1px 7px;border-radius:9px;border:1px solid var(--line)}} .prio.critical{{color:var(--bad);border-color:var(--bad)}} .prio.high{{color:var(--warn);border-color:var(--warn)}}
.case{{display:inline-block;font:11px ui-monospace,Menlo,monospace;padding:1px 6px;margin:1px 3px 1px 0;border-radius:6px;text-decoration:none}}
.case.agent{{background:color-mix(in srgb,var(--ok) 15%,transparent);color:var(--ok)}} .case.manual{{background:color-mix(in srgb,var(--mut) 15%,transparent);color:var(--mut)}}
.gap{{color:var(--bad);font-size:12px}} .review{{font-size:10px;color:var(--warn);text-transform:uppercase}} details{{margin-top:8px}} summary{{cursor:pointer;font-weight:600}}
</style></head><body><main>
<h1>{e(doc["title"])}</h1><div class="muted">From <code>tests/feature-map/</code>, cases in <code>tests/cases/</code>, scenarios in <code>tests/scenarios/</code>.
Rebuild: <code>tests/tools/feature-map.py render</code></div>
<div class="stats">
<div class="stat">Features<b>{n}</b><span class="muted small">{agent_n} agent · {n - agent_n} manual</span></div>
<div class="stat">Covered by any case<b>{covered}/{n}</b><div class="bar"><i style="width:{100 * covered // max(n, 1)}%"></i></div></div>
<div class="stat">Agent features with agent cases<b>{agent_ok}/{agent_n}</b><div class="bar"><i style="width:{100 * agent_ok // max(agent_n, 1)}%"></i></div></div>
<div class="stat">Critical covered<b>{crit_ok}/{len(crit)}</b><div class="bar"><i style="width:{100 * crit_ok // max(len(crit), 1)}%"></i></div></div>
<div class="stat">Cases · scenarios<b>{len(cases)} · {len(scenarios)}</b><span class="muted small">{sum(c["run"] == "agent" for c in cases.values())} agent cases</span></div>
</div>
<details><summary>About this map</summary><div class="small">{"<br>".join(e(x) for x in doc["about"].splitlines())}</div></details>
<h2>Features</h2>
<div class="filters"><input id="q" placeholder="Find a feature, id or axis"><select id="fa"><option value="">All areas</option>{opts((a["prefix"], a["name"]) for a in doc["areas"])}</select>
<select id="fp"><option value="">Any priority</option>{opts((p, p) for p in PRIORITIES)}</select><select id="fc"><option value="">Agent and manual</option>{opts([("agent", "agent"), ("manual", "manual")])}</select>
<select id="fv"><option value="">Any coverage</option>{opts([("none", "not covered"), ("agent", "agent case"), ("manual", "manual case only")])}</select><span id="count" class="muted small"></span></div>
<table id="feats"><thead><tr><th>ID</th><th>Feature</th><th>Priority</th><th>Check</th><th>Axes</th><th>Cases</th></tr></thead><tbody>{"".join(rows)}</tbody></table>
<h2>Scenarios</h2><ul>{scen}</ul>
<h2>State axes</h2><table><thead><tr><th>Axis</th><th>Values</th><th>Where in the code</th></tr></thead><tbody>{"".join(axes_html)}</tbody></table>
<h2>Combinations that matter</h2><table><thead><tr><th>Axes to cover pairwise</th><th>Why</th></tr></thead><tbody>{combos}</tbody></table>
<h2>Hostile content</h2><p class="small">{e(doc["hostile"]["about"])}</p><ul>{hostile}</ul>
</main><script>
const $=s=>document.querySelector(s), rows=[...document.querySelectorAll('#feats tbody tr')];
function apply(){{const q=$('#q').value.toLowerCase(),a=$('#fa').value,p=$('#fp').value,c=$('#fc').value,v=$('#fv').value;let n=0;const vis=new Set();
for(const r of rows){{if(r.classList.contains('area-head'))continue;const ok=(!a||r.dataset.area===a)&&(!p||r.dataset.prio===p)&&(!c||r.dataset.check===c)&&(!v||r.dataset.cov===v)&&(!q||r.textContent.toLowerCase().includes(q));
r.hidden=!ok;if(ok){{n++;vis.add(r.dataset.area)}}}}
for(const r of rows)if(r.classList.contains('area-head'))r.hidden=!vis.has(r.dataset.area);$('#count').textContent=n+' shown'}}
for(const id of ['q','fa','fp','fc','fv'])$('#'+id).addEventListener('input',apply);apply();
</script></body></html>"""
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(page, encoding="utf-8")
    return covered, n


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    doc, cases, scenarios = load()
    if cmd == "check":
        ps = problems(doc, cases, scenarios)
        for p in ps:
            print(p)
        print(f"{len(doc['features'])} features, {len(cases)} cases, {len(scenarios)} scenarios: {len(ps)} problem(s)")
        sys.exit(1 if ps else 0)
    if cmd == "render":
        out = Path(sys.argv[2]) if len(sys.argv) > 2 else TESTS / "reports" / "feature-map.html"
        covered, n = render(doc, cases, scenarios, out)
        print(f"{out}: {covered}/{n} features covered")
        return
    sys.exit(__doc__)


if __name__ == "__main__":
    main()
