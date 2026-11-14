// ── Yolo runs (#603): every run Claude did on its own from My work's Claude dialog — on a task or on an item of Needs
// you now —, newest first (`yolo::runs`, `/api/yolo`), with the filters in one row above the list. Opened from My
// work's header and from ⌘K (`#yolo`); not an activity of the bar.

ACTIVITIES.push({ id: 'yolo', label: 'Yolo runs', layout: 'single', when: () => false, draw: (p) => drawYolo(p), icon: '' })
S.yo = { data: null, at: 0, view: false, f: { state: '', task: '', thread: '', from: '', to: '', changed: false, q: '' }, logs: {} }

// ── yolo runs: the list's filters and words (no DOM: `yolo-runs.test.mjs`)
const YO_STATES = [['', 'All states'], ['working', 'Working'], ['done', 'Done'], ['nothing', "Didn't do it"], ['failed', 'Failed']]
const YO_STATE_WORD = { working: 'Working…', done: 'Done', nothing: "Didn't do it", failed: 'Failed' }
const YO_PERMISSIONS = { acts: 'Acted on its own', asks: 'Prepared and asked' }
/** What a run was on, as the Task filter keeps it: its task, or its item of Needs you now (an older run: its title). */
const yoKey = (r) => r.kind === 'needs' ? `item:${r.item || r.title}` : r.project && r.node ? `task:${r.project}/${r.node}` : `title:${r.title}`
const yoPad = (n) => String(n).padStart(2, '0')
/** A time's day where the user is (YYYY-MM-DD), as the date boxes give it. */
const yoDay = (iso) => { const d = new Date(iso); return `${d.getFullYear()}-${yoPad(d.getMonth() + 1)}-${yoPad(d.getDate())}` }
/** The runs the filters let through (`f`: state, task, thread, from, to — days, inclusive —, changed, q). */
function yoShown(runs, f) {
  const q = (f.q || '').trim().toLowerCase()
  return (runs || []).filter((r) => (!f.state || r.state === f.state)
    && (!f.task || yoKey(r) === f.task)
    && (!f.thread || r.thread === f.thread)
    && (!f.from || yoDay(r.started) >= f.from)
    && (!f.to || yoDay(r.started) <= f.to)
    && (!f.changed || !!r.changed)
    && (!q || `${r.title || ''} ${r.say || ''}`.toLowerCase().includes(q)))
}
/** The Task and Thread filters' choices, from the runs: `{tasks: [[key, title]], threads: [[id, name]]}`, each once. */
function yoChoices(runs) {
  const tasks = new Map(), threads = new Map()
  for (const r of runs || []) {
    if (!tasks.has(yoKey(r))) tasks.set(yoKey(r), r.title || '')
    if (r.thread && !threads.has(r.thread)) threads.set(r.thread, r.thread_name || r.thread)
  }
  const byName = (a, b) => a[1].localeCompare(b[1])
  return { tasks: [...tasks].sort(byName), threads: [...threads].sort(byName) }
}
/** Whether any filter is set. */
const yoFiltered = (f) => !!(f.state || f.task || f.thread || f.from || f.to || f.changed || (f.q || '').trim())
/** How long a run took. */
const yoTook = (s) => s == null ? '' : s >= 3600 ? `${Math.floor(s / 3600)} h ${Math.round((s % 3600) / 60)} min` : s >= 60 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${s} s`
const YO_WD = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'], YO_MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
/** When a run started, from `now`: "Today 14:02", "Yesterday 09:10", else "Thu 12 Nov 14:02". */
function yoWhen(iso, now) {
  const d = new Date(iso), tm = `${yoPad(d.getHours())}:${yoPad(d.getMinutes())}`
  const y = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1)
  if (d.toDateString() === now.toDateString()) return `Today ${tm}`
  if (d.toDateString() === y.toDateString()) return `Yesterday ${tm}`
  return `${YO_WD[d.getDay()]} ${d.getDate()} ${YO_MON[d.getMonth()]}${d.getFullYear() === now.getFullYear() ? '' : ` ${d.getFullYear()}`} ${tm}`
}
/** What a run changed and sent with Echo's CLI, as lines: `[word, command, outcome]` — a draft is in its drafts. */
const yoChanges = (r) => (r.changes || []).filter((c) => c.kind === 'change' || c.kind === 'send')
  .map((c) => { const failed = /^failed/.test(c.outcome || ''); return [failed ? (c.kind === 'send' ? 'Tried to send' : 'Tried to change') : c.kind === 'send' ? 'Sent' : 'Changed', c.command || '', failed ? c.outcome : ''] })
// end of the yolo runs block

async function loadYolo() {
  S.yo.at = Date.now()
  try { S.yo.data = await get('api/yolo') } catch { /* app restarting */ }
  if (S.view === 'yolo') render()
}
// Read when the page is entered, again every 5 s while a run works, else every 30 s.
setInterval(() => {
  const here = S.view === 'yolo' && !document.hidden
  const entered = here && !S.yo.view
  S.yo.view = here
  if (!here) return
  const working = ((S.yo.data && S.yo.data.runs) || []).some((r) => r.state === 'working')
  if (entered || !S.yo.data || Date.now() - S.yo.at > (working ? 5000 : 30000)) loadYolo()
}, 1000)

function drawYolo(primary) {
  patch(primary, `<main class="main">${yoloView(new Date())}</main>`)
}
function yoloView(now) {
  const d = S.yo.data, f = S.yo.f
  const head = `<header class="tk-head yo-head"><h2>Yolo runs</h2><span class="tk-sub">What Claude did on its own · the last ${(d && d.keep_days) || 90} days</span><span class="grow"></span><button class="link-btn" data-go="today">Today</button></header>`
  if (!d) return `<div class="yo">${head}<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(4)}</div></div>`
  const runs = d.runs || []
  if (!runs.length) return `<div class="yo">${head}<div class="wk-empty"><b>No Yolo runs yet</b><p>Give a task, or an item of Needs you now, to Claude with Yolo (✦ on Today): what it did on its own shows here.</p><button class="btn" data-go="today">Open Today</button></div></div>`
  const ch = yoChoices(runs)
  const chip = (key, label, all, opts) => {
    const cur = opts.find(([k]) => k === f[key])
    return `<label class="fchip${f[key] ? ' on' : ''}">${esc(cur && f[key] ? cur[1] : all)} ▾<select data-yo-f="${key}" aria-label="${label}"><option value="">${esc(all)}</option>${opts.filter(([k]) => k).map(([k, l]) => `<option value="${esc(k)}"${f[key] === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  }
  const filters = `<div class="yo-filters" role="group" aria-label="Filters">
    ${chip('state', 'State', 'All states', YO_STATES)}${chip('task', 'Task', 'All tasks and items', ch.tasks)}${chip('thread', 'Thread', 'All threads', ch.threads)}
    <label class="yo-date"><span>From</span><input type="date" data-yo-f="from" value="${esc(f.from)}" aria-label="From"></label>
    <label class="yo-date"><span>To</span><input type="date" data-yo-f="to" value="${esc(f.to)}" aria-label="To"></label>
    <label class="yo-check"><input type="checkbox" data-yo-f="changed"${f.changed ? ' checked' : ''}>Changed or sent something</label>
    <input class="yo-q" id="yo-q" type="search" data-yo-f="q" value="${esc(f.q)}" placeholder="Search what Claude said" aria-label="Search the runs">
    ${yoFiltered(f) ? '<button class="link-btn" data-yo-clear>Clear</button>' : ''}</div>`
  const shown = yoShown(runs, f)
  const count = `<div class="tk-sub yo-count">${shown.length === runs.length ? `${runs.length} run${runs.length === 1 ? '' : 's'}` : `${shown.length} of ${runs.length} runs`}</div>`
  const list = shown.length ? `<div class="yo-list" role="list" aria-label="Yolo runs" data-keep-scroll>${shown.map((r) => yoRun(r, now)).join('')}</div>`
    : '<div class="wk-empty"><b>No run matches these filters</b><button class="btn" data-yo-clear>Clear the filters</button></div>'
  return `<div class="yo">${head}${filters}${count}${list}</div>`
}
function yoRun(r, now) {
  const p = r.thread ? (projById(r.thread) || { id: r.thread, name: r.thread_name || r.thread }) : null
  const what = r.kind === 'needs' ? `<span class="yo-t"><span class="yo-kind">Needs you now</span> ${esc(r.title)}</span>`
    : r.project && r.node ? `<button class="link-btn yo-t" data-tk="open" data-project="${esc(r.project)}" data-node="${esc(r.node)}" title="Open the task">${esc(r.title)}</button>`
    : `<span class="yo-t">${esc(r.title)}</span>`
  const thread = p ? `<span class="tk-pj" title="${esc(p.name)}">${projIcon(p, 12)}<span class="tk-pjn">${esc(p.name)}</span></span>` : ''
  const meta = [yoWhen(r.started, now), r.state === 'working' ? 'working now' : yoTook(r.took) ? `took ${yoTook(r.took)}` : ''].filter(Boolean).join(' · ')
  const perm = YO_PERMISSIONS[r.permissions] ? `<span class="yo-perm" title="The permissions it ran with (Settings → AI & minutes)">${YO_PERMISSIONS[r.permissions]}</span>` : ''
  const changes = yoChanges(r).map(([w, c, o]) => `<li><span class="yo-w${o ? ' failed' : ''}">${w}</span> <code>echoctl ${esc(c)}</code>${o ? ` <span class="set-sub">— ${esc(o)}</span>` : ''}</li>`)
  const drafts = (r.drafts || []).map((x) => `<li><span class="yo-w">Drafted</span> ${esc(draftLine(x))}${x.where === 'm365' && /^https:\/\/outlook\.(office|office365|live)\.com\//i.test(x.url || '') ? ` <a class="wp-link" href="${esc(x.url)}" data-ext title="Open it in Outlook">Open the draft ↗</a>` : ''}</li>`)
  const did = changes.length + drafts.length ? `<ul class="yo-did" aria-label="What it changed, sent and drafted">${changes.join('')}${drafts.join('')}</ul>` : r.state === 'working' ? '' : '<div class="set-sub">Changed, sent and drafted nothing.</div>'
  const open = !!S.yo.logs[r.id]
  const log = open ? `<pre class="yo-log" aria-label="Log">${esc(S.yo.logs[r.id] === true ? 'Reading…' : S.yo.logs[r.id])}</pre>` : ''
  return `<article class="yo-run ${esc(r.state)}" role="listitem" aria-label="${esc(`${YO_STATE_WORD[r.state] || r.state}: ${r.title}`)}">
    <div class="yo-h"><span class="yo-st"><span class="yo-dot" aria-hidden="true"></span>${YO_STATE_WORD[r.state] || esc(r.state)}</span><span class="yo-meta">${esc(meta)}</span>${perm}<span class="grow"></span>${r.log ? `<button class="btn ghost yo-logbtn" data-yo-log="${esc(r.id)}" aria-expanded="${open}">${open ? 'Hide the log' : 'Log'}</button>` : ''}</div>
    <div class="yo-on">${what}${thread}</div>
    ${r.say ? `<div class="yo-say">${esc(r.say)}</div>` : ''}${did}${log}</article>`
}

document.addEventListener('click', async (e) => {
  const t = e.target.closest && e.target.closest('[data-yolo-runs],[data-yo-clear],[data-yo-log]')
  if (!t) return
  if (t.dataset.yoloRuns !== undefined) return go('yolo')
  if (t.dataset.yoClear !== undefined) { S.yo.f = { state: '', task: '', thread: '', from: '', to: '', changed: false, q: '' }; return render() }
  const id = t.dataset.yoLog
  if (S.yo.logs[id]) { delete S.yo.logs[id]; return render() }
  S.yo.logs[id] = true
  render()
  let v = null
  try { v = await get(`api/yolo/log?id=${encodeURIComponent(id)}`) } catch { /* app restarting */ }
  if (!S.yo.logs[id]) return
  S.yo.logs[id] = v && typeof v.log === 'string' ? v.log : (v && v.error) || "The log can't be read now."
  render()
})
const yoSet = (e) => {
  const t = e.target
  if (!t || !t.dataset || t.dataset.yoF === undefined) return
  S.yo.f[t.dataset.yoF] = t.type === 'checkbox' ? t.checked : t.value
  render()
}
document.addEventListener('change', yoSet)
document.addEventListener('input', (e) => { if (e.target && e.target.id === 'yo-q') yoSet(e) })
