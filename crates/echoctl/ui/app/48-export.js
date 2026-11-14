// ── Export a dashboard as an HTML file: Today, a thread's Brief, a client's Overview and their own dashboards have ⤓
// beside ✎. One file, built here: Echo's head (the page's title, when), the widgets as drawn now (their controls taken
// out, charts kept), Echo's own styles, then each widget's data as tables. No script, nothing loaded from outside. The
// app saves it into Downloads under a name of its own and shows it there (`window::export`).

/** ⤓ beside a page's ✎ (not while editing): `page` 'today' or a dashboard's key (`dashPageKey`). */
function exportBtn(page, what) {
  return `<button class="icon-btn ex-btn" data-export="${esc(page)}" aria-label="Export as HTML" title="${esc(`Export as HTML — ${what} as it shows now, with its data, saved to Downloads`)}">${ICON('<path d="M12 4v11"/><path d="M7.5 10.5L12 15l4.5-4.5"/><path d="M5 19.5h14"/>')}</button>`
}

// ── export file: a page's snapshot as one HTML file, and its widgets' data as tables (no DOM: `export.test.mjs`) ──
const EX_TIME = { hour: '2-digit', minute: '2-digit' }
const exHm = (iso) => { const d = iso ? new Date(iso) : null; return d && !Number.isNaN(+d) ? d.toLocaleTimeString([], EX_TIME) : '' }
const exText = (v) => (v == null ? '' : typeof v === 'object' && !Array.isArray(v) ? exText(v.text) : String(v))
const EX_ANSWER = { accepted: 'going', organizer: 'organizer', tentativelyAccepted: 'maybe', notResponded: 'no reply', declined: 'declined' }

/** A plugin widget's data (`kind`, `data`) as tables: `[{cols, rows}]`, a stat's and a card's as label / value rows;
 *  a page draws itself (nothing to list). Data it can't read: a line saying so. */
function exportPluginTables(kind, data) {
  const d = data && typeof data === 'object' ? data : {}
  const kv = (pairs) => ({ cols: ['', ''], rows: pairs.filter(([, v]) => v != null && v !== '').map(([k, v]) => [k, exText(v)]) })
  if (kind === 'stat') return [kv([['Value', `${exText(d.value)}${d.unit ? ` ${exText(d.unit)}` : ''}`], ['What', d.label], ['Progress', d.progress ? `${exText(d.progress.used)} of ${exText(d.progress.of)}` : ''], ['Note', d.note]])]
  if (kind === 'stats') return [{ cols: ['', 'Value', 'Trend'], rows: (d.items || []).map((x) => [exText(x && x.label), exText(x && x.value), Array.isArray(x && x.trend) ? x.trend.join(', ') : '']) }]
  if (kind === 'list') return [{ cols: ['Title', 'Detail'], rows: (d.items || []).map((x) => [exText(x && x.title), exText(x && x.sub)]) }]
  if (kind === 'status') return [kv([['State', d.state], ['Title', d.title], ['Text', d.text]])]
  if (kind === 'card') return [kv([['Title', d.title], ['Text', d.text]])]
  if (kind === 'chart') {
    const series = Array.isArray(d.series) ? d.series : null
    const head = d.headline && d.headline.value != null ? [kv([[d.headline.label || 'Headline', d.headline.value], ['Change', d.headline.change ? `${exText(d.headline.change.by)}${d.headline.change.than ? ` vs ${exText(d.headline.change.than)}` : ''}` : '']])] : []
    const unit = d.unit ? ` (${exText(d.unit)})` : ''
    const cols = ['', ...(series ? series.map((s) => `${exText(s && (s.label || s.key))}${unit}`) : [`Value${unit}`])]
    const rows = (d.points || []).map((p) => [exText(p && p.label), ...(series ? series.map((s) => exText(p && p.values && p.values[s.key])) : [exText(p && p.value)])])
    return [...head, { cols, rows }]
  }
  if (kind === 'table') {
    const cols = (d.columns || []).filter((c) => c && c.key)
    const groups = Object.fromEntries((d.groups || []).map((g) => [String(g && g.id), exText(g && g.label)]))
    const grouped = (d.rows || []).some((r) => r && r.group != null)
    return [{ cols: [...(grouped ? ['Group'] : []), ...cols.map((c) => exText(c.label) || c.key)], rows: (d.rows || []).map((r) => [...(grouped ? [r && r.group != null ? groups[String(r.group)] || String(r.group) : ''] : []), ...cols.map((c) => exText(((r && r.cells) || {})[c.key]))]) }]
  }
  return []
}

/** One of Echo's own widgets (`id`) as tables, from what it is drawn from: `v` its data, `names` thread id → name. */
function exportCoreTables(id, v, names = {}) {
  const x = v || {}
  const thread = (id2) => names[id2] || id2 || ''
  if (['core:calendar.agenda', 'core:calendar.calendar', 'core:calendar.strip'].includes(id)) {
    return [{ cols: ['Time', 'What', 'Kind', 'Threads', 'Answer', 'Minutes'], rows: (x.day || []).map((e) => [[exHm(e.start), exHm(e.end)].filter(Boolean).join('–'), exText(e.subject || e.title), exText(e.kind), (e.threads || []).map((t) => t.name).join(', '), EX_ANSWER[e.response] || exText(e.response), exText(e.minutes && e.minutes.state)]) }]
  }
  if (id === 'core:calendar.next') {
    const n = x.next
    if (!n) return []
    return [{ cols: ['', ''], rows: [['Meeting', exText(n.subject)], ['Time', [exHm(n.start), exHm(n.end)].filter(Boolean).join('–')], ['Threads', (n.threads || []).map((t) => t.name).join(', ')], ['People', Array.isArray(n.people) ? n.people.map((p) => p.name).join(', ') : exText(n.people)], ['Answer', EX_ANSWER[n.response] || exText(n.response)]].filter(([, val]) => val) }]
  }
  if (id === 'core:calendar.choices') return [{ cols: ['When', 'Choice', 'Suggested', 'Why'], rows: (x.choices || []).map((c) => [exHm(c.at), c.kind === 'invite' ? `Answer: ${exText(c.item && c.item.subject)}` : `At once: ${(c.members || []).map((m) => m.subject).join(', ')}`, c.kind === 'invite' ? exText(c.suggest) : exText(c.pick && c.pick.subject), exText(c.why)]) }]
  if (id === 'core:tasks.mine') {
    const rows = []
    const add = (list, whose) => { for (const t of (list && list.items) || []) rows.push([exText(t.title), whose(t), thread(t.project), t.late ? `late (${exText(t.due)})` : t.due_guessed ? '' : exText(t.due)]) }
    add(x.mine, () => 'I owe')
    add(x.owed, (t) => `owed to me · ${exText(t.owner)}`)
    add(x.waiting, (t) => `waiting for · ${exText(t.waiting_for && t.waiting_for.who)}`)
    for (const r of x.plugins || []) rows.push([exText(r.title), `from ${exText(r.source_name || r.source)}`, exText(r.where), r.late ? `late (${exText(r.due)})` : exText(r.due)])
    return [{ cols: ['Task', 'Whose', 'Thread', 'Due'], rows }]
  }
  if (id === 'core:tasks.glance') return [{ cols: ['Thread', 'Done', 'To do', 'Waiting', 'Total'], rows: (x.threads || []).map((t) => [exText(t.name), exText(t.done), exText(t.todo), exText(t.waiting), exText(t.total)]) }]
  if (id === 'core:tasks.done') return [{ cols: ['Task', 'Thread'], rows: (x.done || []).map((t) => [exText(t.title), exText(t.thread)]) }]
  if (id === 'core:calls.week') {
    return [{ cols: ['', ''], rows: [['Calls', exText(x.calls || 0)], ['Memos among them', exText(x.memos || 0)], ['Minutes recorded', exText(x.recorded_min || 0)], ['With minutes', exText(x.with_minutes || 0)]] },
      ...((x.days || []).length ? [{ cols: ['Day', 'Calls'], rows: x.days.map((d) => [exText(d.date), exText(d.calls || 0)]) }] : [])]
  }
  if (id === 'core:needs.now') {
    return [{ cols: ['From', 'Where', 'What', 'At'], rows: (((x.needs || {}).items) || []).map((n) => [exText(n.from), exText(n.where), exText(n.line), exHm(n.at)]) },
      ...((x.asks || []).length ? [{ cols: ['Draft', 'Status'], rows: x.asks.map((a) => [exText((a.draft && a.draft.text) || a.title), exText(a.state || a.status)]) }] : [])]
  }
  if (id === 'core:routines.cards' || id.startsWith('core:routines.card.')) return [{ cols: ['Card', 'About', 'Text'], rows: (x.cards || []).map((c) => [exText(c.title), exText(c.about), exText(c.text)]) }]
  return []
}

/** A table as HTML, every cell escaped; a key / value table without a head row. */
function exportTableHtml(t) {
  const head = t.cols.some(Boolean) ? `<thead><tr>${t.cols.map((c) => `<th scope="col">${esc(c)}</th>`).join('')}</tr></thead>` : ''
  const rows = t.rows.map((r) => `<tr>${r.map((c, i) => (i === 0 && !t.cols[0] ? `<th scope="row">${esc(c)}</th>` : `<td>${esc(c)}</td>`)).join('')}</tr>`).join('')
  return t.rows.length ? `<table class="ex-t">${head}<tbody>${rows}</tbody></table>` : '<p class="set-sub">Nothing in it.</p>'
}

/** The whole file: `title` (the page's), `at` (when, as written), `css` (Echo's styles), `body` (the widgets' markup,
 *  already stripped), `data` ([{label, tables}]). The page's own CSP lets in no script and nothing from outside. */
function exportHtml({ title, at, css, body, data }) {
  // No `<` in a style (a CSS escape in its place): nothing in it can close its tag.
  const style = (s) => String(s || '').replace(/</g, '\\3c ')
  const sections = data.map((w) => `<section class="ex-w"><h3>${esc(w.label)}</h3>${w.tables.length ? w.tables.map(exportTableHtml).join('') : '<p class="set-sub">Drawn above; no data to list.</p>'}</section>`).join('')
  return `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; img-src data:">
<title>${esc(`${title} — Echo`)}</title>
<style>${style(css)}</style>
<style>${EXPORT_CSS}</style>
</head><body class="ex"><div class="ex-page">
<header class="ex-head"><span class="ex-brand">Echo</span><h1>${esc(title)}</h1><span class="set-sub">${esc(at)}</span></header>
<div class="ex-view">${body}</div>
<section class="ex-data" aria-label="Data"><h2>Data</h2>${sections || '<p class="set-sub">No widgets on this page.</p>'}</section>
</div></body></html>
`
}
// What the file adds to Echo's styles: the page scrolls as a document, the widgets at their whole height.
const EXPORT_CSS = `html, body.ex { height: auto; overflow: auto; }
body.ex { background: var(--bg); }
.ex-page { max-width: 1240px; margin: 0 auto; padding: 28px 24px 56px; }
.ex-head { display: flex; align-items: baseline; flex-wrap: wrap; gap: 6px 14px; padding-bottom: 14px; margin-bottom: 18px; border-bottom: 1px solid var(--line); }
.ex-brand { font-weight: 700; color: var(--accent); letter-spacing: 0.02em; }
.ex-head h1 { margin: 0; font-size: 20px; }
.ex-view .td, .ex-view .dash { height: auto !important; max-height: none !important; overflow: visible !important; padding: 0; }
.ex-data { margin-top: 32px; }
.ex-data h2 { font-size: 16px; margin: 0 0 12px; }
.ex-w { background: var(--surface); border: 1px solid var(--line); border-radius: var(--r-panel); padding: 12px 14px; margin-bottom: 12px; }
.ex-w h3 { margin: 0 0 8px; font-size: 13px; }
.ex-t { border-collapse: collapse; width: 100%; margin-bottom: 8px; font-variant-numeric: tabular-nums; }
.ex-t th, .ex-t td { text-align: left; vertical-align: top; padding: 5px 10px 5px 0; border-bottom: 1px solid var(--line); }
.ex-t thead th { color: var(--text-2); font-weight: 600; font-size: 12px; }
.ex-t tbody th { color: var(--text-2); font-weight: 500; width: 30%; }
@media print { body.ex { background: #fff; } .ex-w { break-inside: avoid; } }
`
// end of the export file block

/** The page's title in the file and its name: "Today", "Acme rollout · Brief", "Acme · Overview", "Acme · Staffing". */
function exportTitle(page) {
  if (page === 'today') return 'Today'
  const [kind, rest] = [page.slice(0, page.indexOf(':')), page.slice(page.indexOf(':') + 1)]
  const [id, board] = rest.split('/')
  const list = (S.pj.data && (kind === 'client' ? S.pj.data.clients : S.pj.data.projects)) || []
  const name = (list.find((x) => x.id === id) || {}).name || id
  const b = board && boardsOf(kind).find((x) => x.id === board)
  return `${name} · ${b ? b.name : kind === 'client' ? 'Overview' : 'Brief'}`
}

/** The page's widgets drawn now, in its layout's order: `[{label, tables}]`. */
function exportData(page, catalog) {
  const today = page === 'today', d = (today ? S.today : S.dash) || {}
  const parts = (today ? S.tdParts : S.dashParts) || {}
  const names = Object.fromEntries((((S.pj.data && S.pj.data.projects) || [])).map((p) => [p.id, p.name]))
  const label = (id) => ((d.plugins || {})[id] || {}).label || (catalog.find((w) => w.id === id) || {}).label || id
  // Echo's own: what each is drawn from (on Today as `todayView` reads it; on a dashboard the page's `core`).
  const own = (id) => (d.scoped || {})[id]
  const core = (id) => {
    if (!today) {
      const v = (d.core || {})[id]
      return id === 'core:calendar.next' ? { next: v && v.next } : v
    }
    if (['core:calendar.agenda', 'core:calendar.calendar', 'core:calendar.strip'].includes(id)) return own(id) || d
    if (id === 'core:calendar.next') return { next: own(id) ? own(id).next : d.next }
    if (id === 'core:calendar.choices') return { choices: typeof dayChoices === 'function' ? dayChoices(d.day, new Date(), {}) : [] }
    if (id === 'core:tasks.mine') return own(id) || d.work
    if (id === 'core:tasks.glance' || id === 'core:tasks.done') return (d.glances || {})[id]
    if (id === 'core:calls.week') return d.calls_week
    if (id === 'core:needs.now') return { needs: d.needs, asks: d.asks }
    const text = (html) => new DOMParser().parseFromString(String(html || ''), 'text/html').body.textContent.trim()
    const cards = (d.cards || []).map((c) => ({ ...c, text: text(c.html) }))
    if (id.startsWith('core:routines.card.')) return { cards: cards.filter((c) => `core:routines.card.${c.slug}` === id) }
    if (id === 'core:routines.cards') { const placed = new Set((d.layout || []).map((i) => i.widget)); return { cards: cards.filter((c) => !placed.has(`core:routines.card.${c.slug}`)) } }
    return null
  }
  // The widgets placed on the page and drawn (Today draws some of Echo's own whether placed or not).
  const ids = [...new Set((d.layout || []).map((i) => i.widget))].filter((id) => parts[id])
  return ids.map((id) => {
    const w = (d.plugins || {})[id]
    let tables = []
    try { tables = w ? (w.kind === 'page' ? [] : exportPluginTables(w.kind, w.data)) : exportCoreTables(id, core(id), names) } catch { tables = [] }
    return { label: label(id), tables }
  })
}

/** The page's widgets as drawn, for the file: scrolled lists at their whole height; Echo's controls, inputs, menus and
 *  cards out; a link out (https) kept, opening apart; Echo's own links and buttons that name something kept as text. */
function exportClone(root) {
  const copy = root.cloneNode(true)
  const was = [root, ...root.querySelectorAll('*')], now = [copy, ...copy.querySelectorAll('*')]
  was.forEach((el, i) => {
    if (el.scrollHeight <= el.clientHeight + 1) return
    if (/auto|scroll/.test(getComputedStyle(el).overflowY)) Object.assign(now[i].style, { maxHeight: 'none', height: 'auto', overflow: 'visible' })
  })
  copy.querySelectorAll('.td-head, .dash-h, .bdh, aside.detail, .menu, [role="menu"], script, style, iframe, input, textarea, select, .td-edit-acts, .skeleton').forEach((x) => x.remove())
  const https = (u) => (/^https:\/\/[^\s"<>\\]+$/.test(u || '') ? u : '')
  const as = (el, tag) => {
    const n = document.createElement(tag)
    if (el.className && typeof el.className === 'string') n.className = el.className
    n.append(...el.childNodes)
    el.replaceWith(n)
    return n
  }
  const out = (n, url) => { n.setAttribute('href', url); n.setAttribute('target', '_blank'); n.setAttribute('rel', 'noopener noreferrer') }
  for (const b of copy.querySelectorAll('button')) {
    const url = https(b.dataset.pwOpen || b.dataset.openUrl)
    if (url) out(as(b, 'a'), url)
    else if (/(^|\s)(btn|icon-btn|pw-chev|tab)(\s|$)/.test(b.className) || !b.textContent.trim()) b.remove()
    else as(b, 'span')
  }
  for (const a of copy.querySelectorAll('a')) {
    const url = https(a.getAttribute('href'))
    if (url) out(a, url)
    else as(a, 'span')
  }
  for (const img of copy.querySelectorAll('img')) if (!/^data:image\//.test(img.getAttribute('src') || '')) img.remove()
  for (const el of [copy, ...copy.querySelectorAll('*')]) {
    for (const at of [...el.attributes]) if (/^(data-|on)|^(tabindex|draggable|contenteditable|aria-current|aria-expanded|aria-haspopup|aria-pressed)$/.test(at.name) || (at.name === 'role' && at.value === 'button')) el.removeAttribute(at.name)
  }
  return copy.outerHTML
}

/** ⤓: the page as it shows now, with its data, to the app (it saves the file into Downloads and says where). */
async function exportPage(page) {
  const root = page === 'today' ? $('.td') : $('section.dash')
  if (!root) return window.echo.notice('Nothing to export on this page yet.')
  const title = exportTitle(page), at = new Date()
  const body = exportClone(root)
  let css = '', catalog = S.tdCatalog || []
  try { css = await (await fetch('assets/app.css')).text() } catch { /* the file without Echo's styles */ }
  if (!catalog.length) try { catalog = await get('api/widgets') } catch { /* labels by id */ }
  const when = at.toLocaleString([], { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric', hour: '2-digit', minute: '2-digit' })
  send({ cmd: 'export_save', title, html: exportHtml({ title, at: `Snapshot of ${when}`, css, body, data: exportData(page, catalog) }) })
}

document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-export]')
  if (t) exportPage(t.dataset.export)
})
