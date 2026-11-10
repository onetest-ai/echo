#!/usr/bin/env python3
"""Echo's functional map: tests/feature-map/ (JSON for agents) → checks and an HTML page (for people).

The map is split so features of different parts of the app can be worked on in parallel without merge conflicts:
  areas/<prefix>.json   one area: {prefix, name, order, status, notes, features: [{id, feature, priority, check, axes}]}
  axes/<entity>.json    one entity's state axes: {entity, axes: [{axis, values, where}]}
  combinations.json     axes to cover pairwise, and why
  hostile.json          where others' text reaches the window
  about.md              what the map is and how to keep it

  tests/tools/feature-map.py check            ids, areas and axes consistent; every case's requirements known;
                                              scenarios' cases exist (exit 1 on a problem)
  tests/tools/feature-map.py render [OUT]     the map as one HTML page with coverage (default
                                              tests/reports/feature-map.html)
  tests/tools/feature-map.py changes [BASE]   what this branch changed in the map against BASE (default main): the
                                              features added, changed and removed — the PR's "Map:" line — and a
                                              warning when app code changed but the map didn't, and the
                                              cases, scenarios and map lines that still quote a label or message
                                              the branch took out of the app (exit 1)

  tests/tools/feature-map.py auto-report RESULTS   after an autotest run (tests/auto, `npm test`): its results against the
                                              map — what was tested, what failed and which case to re-run, each area's
                                              coverage (autotests, agent cases, person cases, none), and what changed
                                              since the last run — to tests/reports/auto-<time>.md

Coverage: a case lists the features it checks in its frontmatter (`requirements: [F-CALLS-01, …]`); a scenario lists
its cases (`cases:`). A feature is covered by agent cases, person-run cases, or none.
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
        cases[p.relative_to(TESTS.parent).as_posix()] = {"area": p.parent.name, "id": fm.get("id", ""), "title": fm.get("title", ""),
                                                   "run": fm.get("run", ""), "requirements": fm.get("requirements", []) or []}
    scenarios = {}
    for p in sorted((TESTS / "scenarios").glob("S-*.md")):
        fm = frontmatter(p)
        scenarios[p.relative_to(TESTS.parent).as_posix()] = {"id": fm.get("id", ""), "title": fm.get("title", ""), "cases": fm.get("cases", []) or []}
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
        if f["priority"] not in PRIORITIES or f["check"] not in ("agent", "person"):
            out.append(f"{f['id']}: priority/check must be one of {PRIORITIES} / agent, person")
        out += [f"{f['id']}: unknown axis {x}" for x in f["axes"] if x not in axes]
    for c in doc["combinations"]:
        out += [f"combination: unknown axis {x}" for x in c["axes"] if x not in axes]
    for path, c in cases.items():
        out += [f"{path}: unknown requirement {r}" for r in c["requirements"] if r not in seen]
    for path, s in scenarios.items():
        out += [f"{path}: no such case {c}" for c in s["cases"] if c not in cases]
    by_id = case_ids(cases)
    for spec, ids in auto_links().items():
        out += [f"{spec}: no such case {i}" for i in ids if not i.startswith("F-") and i not in by_id]
        out += [f"{spec}: unknown feature {i}" for i in ids if i.startswith("F-") and i not in seen]
    return out


AUTO = TESTS / "auto"


def auto_links():
    """Each autotest file's links (`covers('area/TC-NNN', 'F-…', …)`): {file: [ids]}."""
    out = {}
    for p in sorted(AUTO.glob("*.spec.mjs")):
        ids = [i for m in re.finditer(r"covers\(([^)]*)\)", p.read_text(encoding="utf-8")) for i in re.findall(r"'([^']+)'", m.group(1))]
        out[p.relative_to(TESTS.parent).as_posix()] = ids
    return out


def case_ids(cases):
    """`area/TC-NNN` → the case's path."""
    return {f"{c['area']}/{c['id']}": path for path, c in cases.items()}


def coverage(doc, cases):
    by = {f["id"]: {"agent": [], "person": []} for f in doc["features"]}
    for path, c in cases.items():
        for r in c["requirements"]:
            if r in by and c["run"] in ("agent", "person"):
                by[r][c["run"]].append(path)
    return by


def render(doc, cases, scenarios, out):
    """One self-contained page: an explorer tree (by area, by scenario, gaps) and a detail pane."""
    cov = coverage(doc, cases)
    data = {
        "title": doc["title"], "about": doc["about"],
        "areas": doc["areas"],
        "features": [{**f, "agent": cov[f["id"]]["agent"], "person": cov[f["id"]]["person"]} for f in doc["features"]],
        "axes": {a["axis"]: {"entity": a["entity"], "values": a["values"], "where": a["where"]} for a in doc["axes"]},
        "combinations": doc["combinations"], "hostile": doc["hostile"],
        "cases": cases, "scenarios": scenarios,
    }
    page = PAGE.replace("__DATA__", json.dumps(data, ensure_ascii=False).replace("</", "<\\/")).replace("__TITLE__", html.escape(doc["title"]))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(page, encoding="utf-8")
    covered = sum(bool(cov[f["id"]]["agent"] or cov[f["id"]]["person"]) for f in doc["features"])
    return covered, len(doc["features"])


PAGE = r"""<!doctype html><html lang="en"><head><meta charset="utf-8"><title>__TITLE__</title>
<meta name="viewport" content="width=device-width,initial-scale=1">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Material+Symbols+Rounded:opsz,wght,FILL,GRAD@20..24,400,0..1,0&display=block"><style>
:root{--accent:#8b62ff;--accent-soft:rgb(139 98 255 / .12);--accent-ink:#5b34d6;--bg:#f6f6f8;--surface:#fff;--sidebar:#efeff3;
--line:rgb(17 17 34 / .08);--line-strong:rgb(17 17 34 / .14);--text:#18181d;--text-2:#55555f;--text-3:#8a8a95;--warn:#b25b00;--danger:#c4314b;
--ok:#0ca30c;--man:#9a9aa3;--none:#d03b3b;--r-control:6px;--r-panel:10px;
--shadow:0 1px 2px rgb(30 20 70 / .06),0 6px 24px rgb(30 20 70 / .06);
--font:-apple-system,BlinkMacSystemFont,"Segoe UI Variable Text","Segoe UI",system-ui,sans-serif;--mono:ui-monospace,"SF Mono","Cascadia Mono",Menlo,Consolas,monospace;color-scheme:light dark}
@media (prefers-color-scheme:dark){:root{--accent:#9b78ff;--accent-soft:rgb(155 120 255 / .16);--accent-ink:#c4b0ff;--bg:#141418;--surface:#1b1b21;--sidebar:#17171c;
--line:rgb(255 255 255 / .07);--line-strong:rgb(255 255 255 / .13);--text:#ececf1;--text-2:#a9a9b4;--text-3:#74747f;--warn:#f0a24a;--danger:#ff6b81;--man:#6f6f79;
--shadow:0 1px 2px rgb(0 0 0 / .3),0 8px 28px rgb(0 0 0 / .35)}}
*{box-sizing:border-box} html,body{height:100%} body{margin:0;font:13px/1.5 var(--font);background:var(--bg);color:var(--text);display:flex;flex-direction:column;-webkit-font-smoothing:antialiased}
.i{font-family:'Material Symbols Rounded';font-weight:normal;font-style:normal;font-size:18px;line-height:1;display:inline-block;flex:none;font-variation-settings:'FILL' 0,'wght' 400,'opsz' 20;-webkit-font-smoothing:antialiased;user-select:none;width:18px;overflow:hidden}
.i.fill{font-variation-settings:'FILL' 1,'wght' 400,'opsz' 20}
.mono{font-family:var(--mono);font-size:11.5px} .mut{color:var(--text-3)} .small{font-size:12px}
header{display:flex;align-items:center;gap:28px;padding:14px 22px;border-bottom:1px solid var(--line);background:var(--surface)}
.brand{display:flex;flex-direction:column;gap:1px} .brand h1{font-size:15px;margin:0;font-weight:650;letter-spacing:-.01em} .brand span{font-size:11.5px;color:var(--text-3)}
.kpis{display:flex;gap:10px;margin-left:auto}
.kpi{display:flex;align-items:center;gap:10px;padding:7px 12px 7px 8px;border:1px solid var(--line);border-radius:var(--r-panel);background:var(--bg)}
.kpi .t{display:flex;flex-direction:column;line-height:1.2} .kpi b{font-size:17px;font-weight:650;letter-spacing:-.02em} .kpi b small{font-size:12px;color:var(--text-3);font-weight:500}
.kpi .l{font-size:11px;color:var(--text-3)}
.meter{height:6px;border-radius:3px;background:var(--line);display:flex;overflow:hidden;gap:2px}
.meter i{display:block;height:100%} .meter i:first-child{border-radius:3px 0 0 3px} .meter i:last-child{border-radius:0 3px 3px 0}
.ok{background:var(--ok)} .man{background:var(--man)} .no{background:var(--none)}
main{flex:1;display:flex;min-height:0}
nav{width:380px;min-width:320px;border-right:1px solid var(--line);display:flex;flex-direction:column;background:var(--sidebar)}
.seg{display:flex;gap:2px;margin:12px 12px 8px;padding:2px;border-radius:8px;background:var(--line)}
.seg button{flex:1;border:0;background:none;color:var(--text-2);border-radius:var(--r-control);padding:5px 6px;font:500 12.5px var(--font);cursor:pointer;display:flex;gap:6px;align-items:center;justify-content:center}
.seg button.on{background:var(--surface);color:var(--text);box-shadow:0 1px 2px rgb(0 0 0 / .08)}
.find{margin:0 12px 10px;position:relative} .find input{width:100%;padding:6px 10px 6px 30px;border:1px solid var(--line-strong);border-radius:var(--r-control);background:var(--surface);color:var(--text);font:inherit;outline:none}
.find input:focus{border-color:var(--accent);box-shadow:0 0 0 3px var(--accent-soft)} .find .i{position:absolute;left:8px;top:6px;color:var(--text-3)}
.tree{flex:1;overflow:auto;padding:0 8px 24px}
.grp{padding:12px 8px 4px;font-size:11px;font-weight:600;text-transform:uppercase;letter-spacing:.06em;color:var(--text-3)}
.row{display:flex;align-items:center;gap:7px;padding:4px 8px;border-radius:var(--r-control);cursor:pointer;white-space:nowrap;color:var(--text-2);min-height:28px}
.row:hover{background:rgb(127 127 140 / .09)} .row.on{background:var(--accent-soft);color:var(--text)} .row.on .lbl b,.row.on .lbl{color:var(--text)}
.tw{width:14px;display:flex;justify-content:center;color:var(--text-3);flex:none} .tw .i{font-size:16px;transition:transform .12s} .tw.open .i{transform:rotate(90deg)}
.lbl{overflow:hidden;text-overflow:ellipsis;flex:1;color:var(--text)} .lbl .k{font-family:var(--mono);font-size:11px;color:var(--text-3);margin-right:5px}
.cnt{color:var(--text-3);font-size:11px;flex:none;font-variant-numeric:tabular-nums} .frac{width:38px;text-align:right} .mini{width:52px;flex:none}
.kids{margin-left:15px;padding-left:6px;border-left:1px solid var(--line-strong)}
.st{width:8px;height:8px;border-radius:50%;flex:none} .st.ok{box-shadow:0 0 0 2px rgb(12 163 12 / .18)} .st.no{box-shadow:0 0 0 2px rgb(208 59 59 / .16)}
.p{font-size:10.5px;font-weight:600;padding:0 6px;border-radius:9px;flex:none;line-height:17px}
.p.critical{color:var(--danger);background:rgb(196 49 75 / .1)} .p.high{color:var(--warn);background:rgb(178 91 0 / .1)} .p.medium,.p.low{color:var(--text-3);background:rgb(127 127 140 / .1)}
section{flex:1;overflow:auto;padding:28px 36px 80px}
.crumb{font-size:12px;color:var(--text-3);display:flex;gap:6px;align-items:center;margin-bottom:6px}
h2{font-size:21px;margin:0 0 6px;font-weight:650;letter-spacing:-.02em} .lead{font-size:15px;line-height:1.55;margin:4px 0 14px;color:var(--text)}
h3{font-size:11px;margin:26px 0 10px;text-transform:uppercase;letter-spacing:.07em;color:var(--text-3);font-weight:600;display:flex;align-items:center;gap:8px}
h3::after{content:"";flex:1;height:1px;background:var(--line)}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(220px,1fr));gap:12px}
.tile{border:1px solid var(--line);border-radius:var(--r-panel);padding:14px;background:var(--surface);cursor:pointer;box-shadow:var(--shadow);display:flex;gap:12px;align-items:center;transition:border-color .12s,transform .12s}
.tile:hover{border-color:var(--accent);transform:translateY(-1px)} .tile .t{min-width:0;flex:1} .tile b{display:block;font-size:13.5px;font-weight:600} .tile .mut{font-size:11.5px}
.tile.rework{opacity:.72}
.ring{flex:none}
.legend{display:flex;gap:16px;align-items:center;margin:4px 0 18px;flex-wrap:wrap;font-size:12px;color:var(--text-2)} .legend span{display:inline-flex;gap:6px;align-items:center}
.card{border:1px solid var(--line);border-radius:var(--r-panel);background:var(--surface);box-shadow:var(--shadow);overflow:hidden}
.card+.card{margin-top:10px}
.list .item{display:flex;gap:10px;align-items:baseline;padding:9px 14px;border-top:1px solid var(--line)} .list .item:first-child{border-top:0}
.list .item:hover{background:rgb(127 127 140 / .05)} .item .id{font-family:var(--mono);font-size:11.5px;white-space:nowrap;min-width:92px}
.item .txt{flex:1} .item .side{color:var(--text-3);font-size:12px;white-space:nowrap}
.tags{display:flex;gap:8px;align-items:center;flex-wrap:wrap;margin-bottom:6px}
.tag{display:inline-flex;align-items:center;gap:6px;padding:3px 9px;border-radius:12px;background:var(--surface);border:1px solid var(--line);font-size:12px;color:var(--text-2)}
a.lnk{color:var(--accent-ink);cursor:pointer;text-decoration:none} a.lnk:hover{text-decoration:underline}
.chip{display:inline-block;padding:2px 8px;border-radius:7px;background:var(--accent-soft);color:var(--accent-ink);margin:2px 4px 2px 0;font:11.5px var(--mono)}
.ax{display:grid;grid-template-columns:minmax(150px,auto) 1fr;gap:2px 16px;padding:10px 14px;border-top:1px solid var(--line)} .ax:first-child{border-top:0}
.ax .where{grid-column:2;font-size:11.5px;color:var(--text-3)}
.empty{color:var(--text-3);padding:4px 0}
code{font:11.5px var(--mono);background:rgb(127 127 140 / .12);padding:1px 5px;border-radius:4px}
.about{font-size:13.5px;color:var(--text-2);line-height:1.65} .about p{margin:0 0 12px} .about ul{margin:0 0 12px;padding-left:20px} .about li{margin:0 0 6px} .about b{color:var(--text);font-weight:600}
.tip{position:fixed;pointer-events:none;background:var(--text);color:var(--bg);padding:6px 9px;border-radius:7px;font-size:12px;max-width:360px;display:none;z-index:9;box-shadow:var(--shadow)}
</style></head><body>
<header><div class="brand"><h1>__TITLE__</h1><span>tests/feature-map · rebuild with <span class="mono">tests/tools/feature-map.py render</span></span></div><div class="kpis" id="kpis"></div></header>
<main><nav><div class="seg"><button data-mode="area" class="on"></button><button data-mode="scen"></button><button data-mode="gaps"></button></div>
<div class="find"><span id="qi"></span><input id="q" placeholder="Find a feature, case or id"></div><div class="tree" id="tree"></div></nav><section id="pane"></section></main>
<div class="tip" id="tip"></div>
<script>
const D=__DATA__;
const I={web:'web',computer:'computer',chev:'chevron_right',folder:'folder',folderOpen:'folder_open',bot:'smart_toy',hand:'back_hand',route:'route',grid:'grid_view',gap:'report',search:'search',feat:'check_circle'};
const FILLED=new Set(['folder','folderOpen','route']);
const ic=(n,st='')=>`<span class="i${FILLED.has(n)?' fill':''}" aria-hidden="true" ${st}>${I[n]}</span>`;
document.querySelector('[data-mode=area]').innerHTML=ic('grid')+'Areas';document.querySelector('[data-mode=scen]').innerHTML=ic('route')+'Scenarios';document.querySelector('[data-mode=gaps]').innerHTML=ic('gap')+'Gaps';
document.getElementById('qi').innerHTML=ic('search');
const esc=s=>String(s??'').replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
const md=s=>esc(s).replace(/`([^`]+)`/g,'<code>$1</code>');
// The about text: paragraphs and "- " lists, their wrapped lines joined; **bold** and `code`.
const inl=s=>md(s).replace(/\*\*([^*]+)\*\*/g,'<b>$1</b>');
function prose(t){const out=[];let list=null,para=[];const flush=()=>{if(para.length){out.push(`<p>${inl(para.join(' '))}</p>`);para=[]}};
 for(const raw of t.split('\n')){const l=raw.trim();
  if(!l){flush();if(list){out.push(`<ul>${list.map(x=>`<li>${inl(x)}</li>`).join('')}</ul>`);list=null}continue}
  if(l.startsWith('- ')){flush();(list=list||[]).push(l.slice(2));continue}
  if(list&&raw.startsWith('  ')){list[list.length-1]+=' '+l;continue}
  para.push(l)}
 flush();if(list)out.push(`<ul>${list.map(x=>`<li>${inl(x)}</li>`).join('')}</ul>`);return out.join('')}
const F=Object.fromEntries(D.features.map(f=>[f.id,f]));
const IN_SCEN=new Set(Object.values(D.scenarios).flatMap(s=>s.cases));
const casesOf=f=>f.agent.concat(f.person);
const cov=f=>casesOf(f).some(p=>IN_SCEN.has(p))?'ok':casesOf(f).length?'man':'no';
const COV={ok:'Checked in a scenario',man:'Checked by a standalone case only',no:'Not covered'};
const st=k=>`<span class="st ${k}" title="${COV[k]}"></span>`;
const prio=p=>`<span class="p ${p}">${p}</span>`;
const scenOf=path=>Object.entries(D.scenarios).filter(([,s])=>s.cases.includes(path)).map(([p,s])=>({path:p,...s}));
const caseName=p=>{const c=D.cases[p];return c?`${c.area}/${c.id}`:p};
const counts=fs=>({o:fs.filter(f=>cov(f)==='ok').length,m:fs.filter(f=>cov(f)==='man').length,n:fs.length});
function meter(fs){const {o,m,n}=counts(fs),x=n-o-m,N=n||1;
 return `<div class="meter" data-tip="${o} in scenarios · ${m} standalone cases only · ${x} not covered">${o?`<i class="ok" style="width:${o/N*100}%"></i>`:''}${m?`<i class="man" style="width:${m/N*100}%"></i>`:''}${x?`<i class="no" style="width:${x/N*100}%"></i>`:''}</div>`}
function ring(fs,size=44){const {o,m,n}=counts(fs),r=size/2-4,C=2*Math.PI*r,N=n||1,seg=(v,off,col)=>v?`<circle r="${r}" cx="${size/2}" cy="${size/2}" fill="none" stroke="${col}" stroke-width="5" stroke-dasharray="${Math.max(v/N*C-2,0.5)} ${C}" stroke-dashoffset="${-off/N*C}" transform="rotate(-90 ${size/2} ${size/2})"/>`:'';
 const pct=Math.round((o+m)/N*100);
 return `<svg class="ring" width="${size}" height="${size}" data-tip="${o} in scenarios · ${m} standalone cases only · ${n-o-m} not covered"><circle r="${r}" cx="${size/2}" cy="${size/2}" fill="none" stroke="var(--line-strong)" stroke-width="5"/>${seg(o,0,'var(--ok)')}${seg(m,o,'var(--man)')}<text x="50%" y="50%" dy=".35em" text-anchor="middle" font-size="11" font-weight="600" fill="var(--text)">${pct}%</text></svg>`}
const S={mode:'area',sel:null,open:new Set(),q:''};
function kpis(){const fs=D.features,ag=fs.filter(f=>f.check==='agent'),cr=fs.filter(f=>f.priority==='critical');
 const k=(l,v,t,list)=>`<div class="kpi">${list?ring(list,38):''}<div class="t"><b>${v}<small> / ${t}</small></b><span class="l">${l}</span></div></div>`;
 document.getElementById('kpis').innerHTML=k('features covered',fs.filter(f=>cov(f)!=='no').length,fs.length,fs)+k('critical covered',cr.filter(f=>cov(f)!=='no').length,cr.length,cr)+k('agent-checkable, with agent case',ag.filter(f=>f.agent.length).length,ag.length,ag.map(f=>({...f,manual:[]})))+
 `<div class="kpi"><div class="t"><b>${Object.keys(D.cases).length}<small> cases</small></b><span class="l">${Object.keys(D.scenarios).length} scenarios · ${[...IN_SCEN].length} cases in scenarios · ${Object.keys(D.cases).length-[...IN_SCEN].length} standalone</span></div></div>`}
const match=f=>!S.q||(f.id+' '+f.feature+' '+f.axes.join(' ')+' '+f.agent.concat(f.person).map(caseName).join(' ')).toLowerCase().includes(S.q);
const tw=(open,has=true)=>`<span class="tw${open?' open':''}">${has?ic('chev'):''}</span>`;
function caseRow(p,n){const c=D.cases[p]||{};return `<div class="row${S.sel==='c:'+p?' on':''}" data-sel="c:${esc(p)}">${tw(false,false)}${n?`<span class="cnt">${n}</span>`:''}${ic(c.run==='agent'?'web':'computer','style="color:var(--text-3)"')}<span class="lbl"><span class="k">${esc(caseName(p))}</span>${esc(c.title)}</span></div>`}
function featRow(f){const on=S.sel==='f:'+f.id,op=S.open.has('f:'+f.id),cs=f.agent.concat(f.person);
 return `<div class="row${on?' on':''}" data-sel="f:${f.id}" data-tog="${cs.length?'f:'+f.id:''}">${tw(op,cs.length)}${st(cov(f))}<span class="lbl" title="${esc(f.feature)}"><span class="k">${f.id.replace('F-','')}</span>${esc(f.feature)}</span>${f.priority==='critical'||f.priority==='high'?prio(f.priority):''}</div>`+(op&&cs.length?`<div class="kids">${cs.map(p=>caseRow(p)).join('')}</div>`:'')}
function tree(){let h='';
 if(S.mode==='area'){for(const s of ['stable','rework']){const as=D.areas.filter(a=>a.status===s);if(!as.length)continue;h+=`<div class="grp">${s==='stable'?'Areas':'Being reworked'}</div>`;
  for(const a of as){const all=D.features.filter(f=>f.area===a.prefix),fs=all.filter(match);if(S.q&&!fs.length)continue;const op=S.open.has('a:'+a.prefix)||!!S.q;
   h+=`<div class="row${S.sel==='a:'+a.prefix?' on':''}" data-sel="a:${a.prefix}" data-tog="a:${a.prefix}">${tw(op)}${ic(op?'folderOpen':'folder','style="color:var(--accent)"')}<span class="lbl"><b style="font-weight:550">${esc(a.name)}</b></span><span class="mini">${meter(all)}</span><span class="cnt frac">${all.filter(f=>cov(f)!=='no').length}/${all.length}</span></div>`;
   if(op)h+=`<div class="kids">${fs.map(featRow).join('')}</div>`}}}
 else if(S.mode==='scen'){h+='<div class="grp">Agent scenarios</div>';for(const [p,s] of Object.entries(D.scenarios)){const op=S.open.has('s:'+p)||!!S.q;
   h+=`<div class="row${S.sel==='s:'+p?' on':''}" data-sel="s:${esc(p)}" data-tog="s:${esc(p)}">${tw(op)}${ic('route','style="color:var(--accent)"')}<span class="lbl"><span class="k">${esc(s.id)}</span>${esc(s.title)}</span><span class="cnt">${s.cases.length}</span></div>`;
   if(op)h+=`<div class="kids">${s.cases.map((c,i)=>caseRow(c,i+1)).join('')}</div>`}
  const man=Object.keys(D.cases).filter(p=>!IN_SCEN.has(p));const op=S.open.has('alone');
  h+=`<div class="grp">Not in a scenario</div><div class="row" data-sel="m:" data-tog="alone">${tw(op)}${ic('hand','style="color:var(--text-3)"')}<span class="lbl">Standalone cases</span><span class="cnt">${man.length}</span></div>`+(op?`<div class="kids">${man.map(p=>caseRow(p)).join('')}</div>`:'')}
 else{for(const pr of ['critical','high','medium','low']){const no=D.features.filter(f=>f.priority===pr&&cov(f)==='no'&&match(f)),ma=D.features.filter(f=>f.priority===pr&&f.check==='agent'&&cov(f)==='man'&&match(f));
   if(!no.length&&!ma.length)continue;h+=`<div class="grp">${pr} · ${no.length} not covered${ma.length?` · ${ma.length} could be agent`:''}</div>`+no.map(featRow).join('')+ma.map(featRow).join('')}}
 document.getElementById('tree').innerHTML=h||'<div class="empty" style="padding:12px">Nothing matches</div>'}
const item=(id,txt,side,lead='')=>`<div class="item">${lead}<span class="id">${id}</span><span class="txt">${txt}</span>${side?`<span class="side">${side}</span>`:''}</div>`;
function pane(){const el=document.getElementById('pane'),[k,v]=(S.sel||':').split(/:(.*)/s);let h='';
 if(!S.sel||k==='m'){h=`<div class="crumb">${ic('grid')} Overview</div><h2>Coverage by area</h2><div class="legend">${['ok','man','no'].map(c=>`<span>${st(c)} ${COV[c]}</span>`).join('')}</div><div class="grid">`+
  D.areas.map(a=>{const fs=D.features.filter(f=>f.area===a.prefix),{o,m,n}=counts(fs);return `<div class="tile${a.status==='rework'?' rework':''}" data-sel="a:${a.prefix}">${ring(fs)}<div class="t"><b>${esc(a.name)}</b><span class="mut">${n} features · ${o} in scenarios · ${m} standalone${a.status==='rework'?' · rework':''}</span></div></div>`}).join('')+'</div>'+
  `<h3>About this map</h3><div class="about">${prose(D.about)}</div>`}
 else if(k==='a'){const a=D.areas.find(x=>x.prefix===v),fs=D.features.filter(f=>f.area===v);
  h=`<div class="crumb">${ic('folder')} Area · F-${esc(v)}${a.status==='rework'?' · being reworked':''}</div><div style="display:flex;gap:16px;align-items:center">${ring(fs,64)}<div><h2>${esc(a.name)}</h2><div class="mut">${esc(a.notes)}</div></div></div>`;
  for(const c of ['no','man','ok']){const g=fs.filter(f=>cov(f)===c);if(!g.length)continue;h+=`<h3>${st(c)} ${COV[c]} · ${g.length}</h3><div class="card list">${g.map(f=>item(`<a class="lnk" data-sel="f:${f.id}">${f.id}</a>`,md(f.feature),prio(f.priority)+' '+(f.check==='agent'?ic('web'):ic('computer')))).join('')}</div>`}}
 else if(k==='f'){const f=F[v],cs=f.agent.concat(f.person),sc=[...new Map(cs.flatMap(scenOf).map(s=>[s.path,s])).values()],combos=D.combinations.filter(c=>c.axes.some(x=>f.axes.includes(x)));
  h=`<div class="crumb">${ic('folder')}<a class="lnk" data-sel="a:${f.area}">${esc((D.areas.find(a=>a.prefix===f.area)||{}).name)}</a> › <span class="mono">${f.id}</span></div><p class="lead">${md(f.feature)}</p>
  <div class="tags"><span class="tag">${st(cov(f))} ${COV[cov(f)]}</span>${prio(f.priority)}<span class="tag">${f.check==='agent'?ic('web')+' Checkable in the served window or CLI':ic('computer')+' Needs a real machine'}</span>${f.review?'<span class="tag">needs review</span>':''}</div>
  <h3>Cases · ${cs.length}</h3>${cs.length?`<div class="card list">${cs.map(p=>item(`<a class="lnk" data-sel="c:${esc(p)}">${esc(caseName(p))}</a>`,esc(D.cases[p].title),'',ic(D.cases[p].run==='agent'?'web':'computer'))).join('')}</div>`:'<div class="empty">No case checks it yet.</div>'}
  <h3>Scenarios · ${sc.length}</h3>${sc.length?`<div class="card list">${sc.map(s=>item(`<a class="lnk" data-sel="s:${esc(s.path)}">${esc(s.id)}</a>`,esc(s.title),'',ic('route'))).join('')}</div>`:'<div class="empty">In no agent scenario.</div>'}
  <h3>State axes · ${f.axes.length}</h3>${f.axes.length?`<div class="card">${f.axes.map(a=>`<div class="ax"><span class="mono">${esc(a)}</span><span>${md((D.axes[a]||{}).values)}</span><span class="where">${md((D.axes[a]||{}).where)}</span></div>`).join('')}</div>`:'<div class="empty">None.</div>'}
  ${combos.length?`<h3>Combinations that involve it</h3>${combos.map(c=>`<div class="card" style="padding:12px 14px"><div>${c.axes.map(a=>`<span class="chip" data-tip="${esc((D.axes[a]||{}).values)}">${esc(a)}</span>`).join('<span class="mut"> × </span>')}</div><div class="small mut" style="margin-top:6px">${md(c.why)}</div></div>`).join('')}`:''}
  <h3>Source</h3><div class="mono mut">tests/feature-map/${esc(f.file)}</div>`}
 else if(k==='c'){const c=D.cases[v];h=`<div class="crumb">${ic(c.run==='agent'?'web':'computer')} ${c.run==='agent'?'Case · served window and CLI':'Case · real machine'} · <span class="mono">${esc(caseName(v))}</span></div><h2>${esc(c.title)}</h2><div class="mono mut">${esc(v)}</div>
  <h3>Checks · ${c.requirements.length}</h3>${c.requirements.length?`<div class="card list">${c.requirements.map(r=>F[r]?item(`<a class="lnk" data-sel="f:${r}">${r}</a>`,esc(F[r].feature),prio(F[r].priority),st(cov(F[r]))):item(esc(r),'<span class="mut">unknown feature</span>')).join('')}</div>`:'<div class="empty">No features linked.</div>'}
  <h3>In scenarios</h3>${scenOf(v).length?`<div class="card list">${scenOf(v).map(s=>item(`<a class="lnk" data-sel="s:${esc(s.path)}">${esc(s.id)}</a>`,esc(s.title),'stage '+(s.cases.indexOf(v)+1),ic('route'))).join('')}</div>`:'<div class="empty">None.</div>'}`}
 else if(k==='s'){const s=D.scenarios[v],fs=[...new Set(s.cases.flatMap(c=>(D.cases[c]||{requirements:[]}).requirements))].filter(r=>F[r]);
  h=`<div class="crumb">${ic('route')} Scenario · <span class="mono">${esc(s.id)}</span> · ${s.cases.length} stages · ${fs.length} features</div><h2>${esc(s.title)}</h2><div class="mono mut">${esc(v)}</div><h3>Stages</h3><div class="card list">${s.cases.map((c,i)=>item(`<a class="lnk" data-sel="c:${esc(c)}">${esc(caseName(c))}</a>`,esc((D.cases[c]||{}).title)+`<div style="margin-top:4px">${((D.cases[c]||{}).requirements||[]).map(r=>`<a class="chip" data-sel="f:${r}" data-tip="${esc((F[r]||{}).feature)}">${r.replace('F-','')}</a>`).join('')}</div>`,'',`<span class="cnt" style="min-width:16px">${i+1}</span>`)).join('')}</div>`}
 el.innerHTML=h;el.scrollTop=0}
function draw(){tree();pane()}
document.addEventListener('click',e=>{const m=e.target.closest('[data-mode]');if(m){S.mode=m.dataset.mode;document.querySelectorAll('.seg button').forEach(b=>b.classList.toggle('on',b===m));return draw()}
 const t=e.target.closest('[data-sel]');if(!t)return;const sel=t.dataset.sel,tog=t.dataset.tog;
 if(tog&&(S.sel===sel||e.target.closest('.tw')))S.open.has(tog)?S.open.delete(tog):S.open.add(tog);else if(tog)S.open.add(tog);
 if(sel.startsWith('f:')){const f=F[sel.slice(2)];if(f)S.open.add('a:'+f.area)}
 S.sel=sel;draw()});
document.getElementById('q').addEventListener('input',e=>{S.q=e.target.value.trim().toLowerCase();tree()});
const tip=document.getElementById('tip');document.addEventListener('mousemove',e=>{const t=e.target.closest('[data-tip]');if(!t||!t.dataset.tip){tip.style.display='none';return}tip.textContent=t.dataset.tip;tip.style.display='block';tip.style.left=Math.min(e.clientX+12,innerWidth-370)+'px';tip.style.top=(e.clientY+14)+'px'});
kpis();draw();
</script></body></html>"""


def auto_results(path):
    """Playwright's JSON results as one row a test: file, title, status (passed, failed, flaky, skipped), seconds,
    its links and its error's first line."""
    data = json.loads(Path(path).read_text(encoding="utf-8"))
    rows = []

    def walk(suite, file):
        file = suite.get("file") or file
        for spec in suite.get("specs", []):
            for t in spec.get("tests", []):
                res = t.get("results") or [{}]
                last = res[-1]
                status = {"expected": "passed", "unexpected": "failed", "flaky": "flaky", "skipped": "skipped"}.get(t.get("status"), t.get("status"))
                err = ((last.get("error") or {}).get("message") or "").strip().splitlines()
                rows.append({"file": spec.get("file") or file, "title": spec["title"], "status": status,
                             "seconds": round(sum(r.get("duration", 0) for r in res) / 1000, 1),
                             "links": [a["description"] for a in t.get("annotations", []) if a.get("type") in ("case", "feature")],
                             "error": re.sub(r"\x1b\[[0-9;]*m", "", err[0]) if err else ""})
        for s in suite.get("suites", []):
            walk(s, file)

    for s in data.get("suites", []):
        walk(s, None)
    return rows, data.get("stats", {})


def auto_report(path):
    """The run's report (tests/reports/auto-<time>.md) and its state for the next one (tests/reports/auto-last.json)."""
    import subprocess
    from datetime import datetime
    doc, cases, _ = load()
    rows, stats = auto_results(path)
    by_id = case_ids(cases)
    names = {f["id"]: f for f in doc["features"]}
    cov = coverage(doc, cases)
    git = lambda *a: subprocess.run(["git", *a], cwd=TESTS.parent, capture_output=True, text=True).stdout.strip()
    commit = git("rev-parse", "--short", "HEAD")
    # A feature is auto-tested when a test names it, or names a case that checks it.
    auto = {}
    for r in rows:
        feats = {i for i in r["links"] if i.startswith("F-")}
        feats |= {f for i in r["links"] if i in by_id for f in cases[by_id[i]]["requirements"]}
        for f in feats:
            auto.setdefault(f, []).append(r)
    state_file = TESTS / "reports" / "auto-last.json"
    prev = json.loads(state_file.read_text()) if state_file.exists() else None
    key = lambda r: f"{r['file']} › {r['title']}"
    now = {key(r): r["status"] for r in rows}
    ok = lambda f: f in auto and all(r["status"] in ("passed", "flaky") for r in auto[f])
    count = {s: sum(1 for r in rows if r["status"] == s) for s in ("passed", "failed", "flaky", "skipped")}
    L = [f"# Autotest run {datetime.now():%Y-%m-%d %H:%M}", "",
         f"Commit `{commit}`{' (uncommitted changes)' if git('status', '--porcelain', '--', 'crates', 'plugin', 'tests') else ''} · "
         f"{len(rows)} tests: {count['passed']} passed, {count['failed']} failed, {count['flaky']} flaky, {count['skipped']} skipped · "
         f"{round(stats.get('duration', 0) / 1000)} s", ""]
    failed = [r for r in rows if r["status"] == "failed"]
    if failed:
        L += ["## Failed — re-run these cases by hand to tell a bug from a broken test", ""]
        for r in failed:
            cs = [by_id[i] for i in r["links"] if i in by_id]
            L += [f"- **{r['title']}** (`{r['file']}`): {r['error'] or 'failed'}", f"  - cases: {', '.join(f'`{c}`' for c in cs) or '(none linked)'}"]
        L.append("")
    flaky = [r for r in rows if r["status"] == "flaky"]
    if flaky:
        L += ["## Flaky (passed on a retry) — a bug in the test or the app, never a footnote", ""] + [f"- {r['title']} (`{r['file']}`)" for r in flaky] + [""]
    L += ["## Coverage by area", "",
          "Features: tested by an autotest (passing / failing), checked only by agent cases, only by a person, or by nothing.", "",
          "| Area | Features | Autotests ✓ | Autotests ✗ | Agent cases only | Person only | Nothing |", "|---|---|---|---|---|---|---|"]
    tot = [0] * 6
    for a in doc["areas"]:
        fs = [f for f in doc["features"] if f["area"] == a["prefix"]]
        n_ok = sum(1 for f in fs if ok(f["id"]))
        n_bad = sum(1 for f in fs if f["id"] in auto and not ok(f["id"]))
        rest = [f for f in fs if f["id"] not in auto]
        n_agent = sum(1 for f in rest if cov[f["id"]]["agent"])
        n_person = sum(1 for f in rest if not cov[f["id"]]["agent"] and cov[f["id"]]["person"])
        n_none = len(rest) - n_agent - n_person
        vals = [len(fs), n_ok, n_bad, n_agent, n_person, n_none]
        tot = [x + y for x, y in zip(tot, vals)]
        L.append(f"| {a['name']} | " + " | ".join(str(v) for v in vals) + " |")
    L.append("| **All** | " + " | ".join(f"**{v}**" for v in tot) + " |")
    L.append("")
    L.append(f"Autotests reach {tot[1] + tot[2]} of {tot[0]} features ({round(100 * (tot[1] + tot[2]) / max(tot[0], 1))}%).")
    L.append("")
    L += ["## What the autotests checked", "", "| Test | Status | s | Cases | Features |", "|---|---|---|---|---|"]
    for r in rows:
        cs = [i for i in r["links"] if not i.startswith("F-")]
        fs = sorted({i for i in r["links"] if i.startswith("F-")} | {f for i in cs if i in by_id for f in cases[by_id[i]]["requirements"]})
        L.append(f"| {r['title']} | {r['status']} | {r['seconds']} | {', '.join(cs)} | {', '.join(fs)} |")
    L.append("")
    top = [f for f in doc["features"] if f["id"] not in auto and f["priority"] in ("critical", "high")]
    L += [f"## Left: critical and high features no autotest reaches ({len(top)})", ""]
    for a in doc["areas"]:
        fs = [f for f in top if f["area"] == a["prefix"]]
        if fs:
            L.append(f"- **{a['name']}**: " + "; ".join(f"{f['id']} {f['feature'][:70]}{'…' if len(f['feature']) > 70 else ''}" for f in fs))
    L.append("")
    L += ["## Since the last run", ""]
    if not prev:
        L += ["The first run: nothing to compare.", ""]
    else:
        before = prev["tests"]
        added = sorted(set(now) - set(before))
        gone = sorted(set(before) - set(now))
        changed = sorted(k for k in set(now) & set(before) if now[k] != before[k])
        got = sorted(f for f in auto if f not in prev["features"])
        lost = sorted(f for f in prev["features"] if f not in auto)
        L.append(f"Last run: {prev['at']} at `{prev['commit']}`.")
        L.append("")
        L += [f"- Tests added: {len(added)}"] + [f"  - {k}" for k in added]
        L += [f"- Tests removed: {len(gone)}"] + [f"  - {k}" for k in gone]
        L += [f"- Status changed: {len(changed)}"] + [f"  - {k}: {before[k]} → {now[k]}" for k in changed]
        L += [f"- Features newly reached: {len(got)}" + (f" ({', '.join(got)})" if got else ""),
              f"- Features no longer reached: {len(lost)}" + (f" ({', '.join(lost)})" if lost else "")]
        # The cases and the map's features changed in git since that run: their tests may need updating too.
        touched = [p for p in git("diff", "--name-only", prev["commit"], "--", "tests/cases", "tests/feature-map").splitlines() if p]
        linked = sorted({p for p in touched if p in cases and any(p == by_id.get(i) for r in rows for i in r["links"])})
        L += [f"- Cases and map files changed since then: {len(touched)}; of them cases an autotest automates (check the test still matches): {len(linked)}"]
        L += [f"  - `{p}`" for p in linked]
        L.append("")
    out = TESTS / "reports" / f"auto-{datetime.now():%Y-%m-%d_%H%M%S}.md"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(L) + "\n", encoding="utf-8")
    state_file.write_text(json.dumps({"at": f"{datetime.now():%Y-%m-%d %H:%M}", "commit": commit, "tests": now, "features": sorted(auto)}, indent=1))
    print(f"{len(rows)} tests: {count['passed']} passed, {count['failed']} failed, {count['flaky']} flaky, {count['skipped']} skipped; "
          f"autotests reach {tot[1] + tot[2]} of {tot[0]} features")
    print(f"report: {out.relative_to(TESTS.parent)}")
    return 1 if failed else 0


APP_CODE = ("crates/", "plugin/")


QUOTED = re.compile(r"""'([^'\\\n]{4,80})'|"([^"\\\n]{4,80})"|`([^`$\\\n]{4,80})`|>([^<>{}$\n]{4,80})<""")


def gone_texts(diff, still_there):
    """Texts the window, CLI or plugin showed that this diff took out: a quoted phrase (two words or more) on a removed
    line that no line of the app holds any more (`still_there(text)` false)."""
    added = "\n".join(l[1:] for l in diff.splitlines() if l.startswith("+") and not l.startswith("+++"))
    out = set()
    for line in diff.splitlines():
        if not line.startswith("-") or line.startswith("---"):
            continue
        for m in QUOTED.finditer(line[1:]):
            t = next(g for g in m.groups() if g is not None).strip()
            if re.fullmatch(r"[A-Z][a-z']*( [A-Za-z'’,.…-]+)+", t) and t not in added and not still_there(t):
                out.add(t)
    return sorted(out)


def quoting(texts):
    """Each line of a case, a scenario, the map's areas or the app profile that quotes one of `texts`: (file, line, text)."""
    hits = []
    for f in sorted(list((TESTS / "cases").rglob("*.md")) + list((TESTS / "scenarios").glob("*.md")) + list((MAP / "areas").glob("*.json")) + [TESTS / "app_profile.md"]):
        for n, line in enumerate(f.read_text(encoding="utf-8").splitlines(), 1):
            hits += [(f.relative_to(TESTS.parent), n, t) for t in texts if t in line]
    return hits


def changes(base):
    """The map's features on BASE vs the working tree, and whether app code changed without the map."""
    import subprocess
    root = TESTS.parent
    git = lambda *a: subprocess.run(["git", *a], cwd=root, capture_output=True, text=True, encoding="utf-8", check=True).stdout
    merge_base = git("merge-base", base, "HEAD").strip()
    old = {}
    for name in git("ls-tree", "--name-only", f"{merge_base}:tests/feature-map/areas").split() if git("ls-tree", merge_base, "tests/feature-map/areas") else []:
        a = json.loads(git("show", f"{merge_base}:tests/feature-map/areas/{name}"))
        old.update({f["id"]: f for f in a["features"]})
    new = {f["id"]: {k: v for k, v in f.items() if k not in ("area", "file")} for f in load_map()["features"]}
    added = sorted(set(new) - set(old))
    removed = sorted(set(old) - set(new))
    changed = sorted(i for i in set(new) & set(old) if new[i] != old[i])
    files = set(git("diff", "--name-only", merge_base).split()) | set(git("ls-files", "--others", "--exclude-standard").split())
    code = sorted(f for f in files if f.startswith(APP_CODE))
    touched_map = any(f.startswith("tests/feature-map/") for f in files)
    def ids(xs):
        if len(xs) <= 12:
            return ", ".join(xs)
        by = {}
        for x in xs:
            by[x.rsplit("-", 1)[0]] = by.get(x.rsplit("-", 1)[0], 0) + 1
        return f"{len(xs)} features ({', '.join(f'{k} {n}' for k, n in by.items())})"
    parts = [f"added {ids(added)}" if added else "", f"changed {ids(changed)}" if changed else "", f"removed {ids(removed)}" if removed else ""]
    line = "; ".join(p for p in parts if p) or "no change"
    print(f"Map: {line}")
    still = lambda t: subprocess.run(["git", "grep", "-qF", t, "--", *APP_CODE], cwd=root).returncode == 0
    stale = quoting(gone_texts(git("diff", "-U0", merge_base, "--", *APP_CODE), still))
    for f, n, t in stale:
        print(f"{f}:{n}: still says \"{t}\", which no line of the app holds any more")
    if stale:
        print("Update those lines in this branch, or say in the PR why the app still shows the text (built from parts).")
    if code and not touched_map:
        print(f"warning: {len(code)} app file(s) changed ({', '.join(code[:5])}{' …' if len(code) > 5 else ''}) but tests/feature-map/ didn't.")
        print("Update the features this changes, or say in the PR why the behaviour doesn't change (\"Map: no behaviour change — <why>\").")
        return 1
    return 1 if stale else 0


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "check"
    doc, cases, scenarios = load()
    if cmd == "check":
        ps = problems(doc, cases, scenarios)
        for p in ps:
            print(p)
        print(f"{len(doc['features'])} features, {len(cases)} cases, {len(scenarios)} scenarios: {len(ps)} problem(s)")
        sys.exit(1 if ps else 0)
    if cmd == "changes":
        sys.exit(changes(sys.argv[2] if len(sys.argv) > 2 else "main"))
    if cmd == "auto-report":
        sys.exit(auto_report(sys.argv[2] if len(sys.argv) > 2 else AUTO / "test-results" / "results.json"))
    if cmd == "render":
        out = Path(sys.argv[2]) if len(sys.argv) > 2 else TESTS / "reports" / "feature-map.html"
        covered, n = render(doc, cases, scenarios, out)
        print(f"{out}: {covered}/{n} features covered")
        return
    sys.exit(__doc__)


if __name__ == "__main__":
    main()
