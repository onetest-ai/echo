// ── Plugins' widgets on Today (`plugin:<plugin>:<widget>`, #315; design: Workbench `WidgetsPluginAPI`,
// `WidgetsPluginKinds`): Echo draws a plugin's data by the widget's kind — its text is data, escaped; a link only to
// echo:// (Echo opens it). The data comes from api/today's `plugins` (the last answer of the plugin's host half).

// ── plugin widget: a kind's data as HTML, or why it can't be drawn (no DOM: `today-plugin-widgets.test.mjs`) ───────
/** `kind` (stat, stats, chart, list, status, card, table) with `data` at `size` → HTML; throws with what is wrong in the
 *  data. `opened`: the ids of a table's rows shown with their details. */
function pluginWidgetHtml(kind, data, size, opened = []) {
  const d = data && typeof data === 'object' ? data : null
  if (!d) throw new Error('its data is not an object')
  const str = (v, what, max = 200) => { if (v == null) return ''; if (typeof v !== 'string' && typeof v !== 'number') throw new Error(`${what} is not text`); return String(v).slice(0, max) }
  const TONES = ['good', 'warn', 'bad']
  const tone = (t) => (TONES.includes(t) ? ` tone-${t}` : '')
  // A link only to Echo's own places; anything else is shown as text.
  const linked = (text, link) => (typeof link === 'string' && /^echo:\/\/[a-z]+\//.test(link) ? `<a class="link-btn" href="${esc(link)}">${esc(text)}</a>` : esc(text))
  // A link of a row or an item: echo:// opens in Echo, https asks first (`data-pw-open`); anything else: none ('').
  const outLink = (text, link) => typeof link === 'string' && /^echo:\/\/[a-z]+\//.test(link) ? `<a class="link-btn" href="${esc(link)}">${esc(text)}</a>`
    : typeof link === 'string' && /^https:\/\/[^\s"<>\\]+$/.test(link) ? `<button type="button" class="link-btn" data-pw-open="${esc(link)}">${esc(text)}</button>` : ''
  const num = (v, what) => { const n = typeof v === 'number' || (typeof v === 'string' && v.trim()) ? Number(v) : NaN; if (!Number.isFinite(n)) throw new Error(`${what} is not a number`); return n }
  // Any kind may say where it opens (`open`: echo:// in Echo, https after a confirm) and up to 3 actions (its plugin's
  // commands, run on a click).
  const foot = () => {
    const bits = []
    if (typeof d.open === 'string' && /^echo:\/\/[a-z]+\//.test(d.open)) bits.push(`<a class="link-btn" href="${esc(d.open)}">Open</a>`)
    else if (typeof d.open === 'string' && /^https:\/\/[^\s"<>\\]+$/.test(d.open)) bits.push(`<button type="button" class="link-btn" data-pw-open="${esc(d.open)}">Open</button>`)
    if (Array.isArray(d.actions)) for (const [i, a] of d.actions.slice(0, 3).entries()) {
      if (!a || typeof a.command !== 'string' || !a.command) throw new Error(`action ${i + 1} needs a command`)
      bits.push(`<button type="button" class="btn sm" data-pw-cmd="${esc(a.command)}" data-pw-args="${esc(JSON.stringify(a.args ?? {}))}">${esc(str(a.label, `action ${i + 1}'s label`, 40) || a.command)}</button>`)
    }
    return bits.length ? `<div class="pw-foot">${bits.join('')}</div>` : ''
  }
  if (kind === 'stat') {
    if (d.value == null) throw new Error('a stat needs value')
    let bar = ''
    if (d.progress != null) {
      const used = num(d.progress.used, 'progress.used'), of = num(d.progress.of, 'progress.of')
      const pct = of > 0 ? Math.max(0, Math.min(100, (used / of) * 100)) : 0
      bar = `<div class="pw-prog" role="img" aria-label="${esc(`${used} of ${of}`)}"><span style="width:${pct.toFixed(1)}%"></span></div>`
    }
    return `<div class="pw-stat"><b class="${tone(d.tone).trim()}">${esc(str(d.value, 'value', 24))}</b>${d.unit ? `<span class="set-sub">${esc(str(d.unit, 'unit', 12))}</span>` : ''}${d.label ? `<span class="set-sub">${esc(str(d.label, 'label', 80))}</span>` : ''}</div>${bar}${d.note && size !== 'S' ? `<div class="set-sub pw-note">${esc(str(d.note, 'note'))}</div>` : ''}${foot()}`
  }
  if (kind === 'chart') return pluginChartHtml(d, size, { str, num, foot })
  if (kind === 'stats') {
    if (!Array.isArray(d.items)) throw new Error('stats need items: [{label, value}]')
    const items = d.items.slice(0, size === 'L' ? 6 : size === 'S' ? 2 : 4)
    // An item may open matching rows elsewhere: "Details ›".
    const more = (x) => (x ? outLink('Details ›', x.link) : '')
    // An item's `trend`: a sparkline under its value (up to 30 numbers).
    const spark = (x, i) => {
      if (!x || x.trend == null) return ''
      if (!Array.isArray(x.trend)) throw new Error(`item ${i + 1}'s trend is not a list of numbers`)
      const t = x.trend.slice(-30).map((v, k) => num(v, `item ${i + 1}'s trend ${k + 1}`))
      if (t.length < 2) return ''
      // The column's width, a soft fill under it, a dot on the last value (the number above it).
      const lo = Math.min(...t), hi = Math.max(...t), span = hi - lo || 1
      const xy = t.map((v, k) => [(k / (t.length - 1)) * 100, 25 - ((v - lo) / span) * 22])
      const line = xy.map(([a, b]) => `${a.toFixed(1)},${b.toFixed(1)}`).join(' ')
      const [lx, ly] = xy[xy.length - 1]
      return `<span class="pc-spark" aria-hidden="true"><svg viewBox="0 0 100 28" preserveAspectRatio="none"><polygon points="0,28 ${line} 100,28"/><polyline points="${line}" fill="none" vector-effect="non-scaling-stroke"/></svg><i style="left:${lx.toFixed(1)}%;top:${((ly / 28) * 100).toFixed(1)}%"></i></span>`
    }
    const trends = items.some((x) => x && Array.isArray(x.trend) && x.trend.length > 1)
    return `<div class="pw-stats${trends ? ' trends' : ''}">${items.map((x, i) => `<div><b class="${tone(x && x.tone).trim()}">${esc(str(x && x.value, `item ${i + 1}'s value`, 24))}</b><span class="set-sub">${esc(str(x && x.label, `item ${i + 1}'s label`, 60))}</span>${spark(x, i)}${more(x)}</div>`).join('')}</div>${foot()}`
  }
  if (kind === 'list') {
    if (!Array.isArray(d.items)) throw new Error('a list needs items: [{title, sub?, link?}]')
    const items = d.items.slice(0, size === 'L' ? 10 : size === 'S' ? 3 : 5)
    if (!items.length) return `<p class="set-sub">${esc(str(d.empty, 'empty') || 'Nothing here.')}</p>${foot()}`
    return `<div class="pw-list" role="list">${items.map((x, i) => `<div class="pw-li" role="listitem"><span>${linked(str(x && x.title, `item ${i + 1}'s title`), x && x.link)}</span>${x && x.sub ? `<span class="set-sub">${esc(str(x.sub, `item ${i + 1}'s sub`))}</span>` : ''}</div>`).join('')}</div>${foot()}`
  }
  if (kind === 'status') {
    if (!['ok', 'warn', 'bad'].includes(d.state)) throw new Error('a status needs state: ok, warn or bad')
    return `<div class="pw-status st-${d.state}"><i aria-hidden="true"></i><b>${esc(str(d.title, 'title', 80))}</b>${d.text ? `<span class="set-sub">${esc(str(d.text, 'text', 300))}</span>` : ''}</div>${foot()}`
  }
  if (kind === 'card') {
    if (d.title == null && d.text == null) throw new Error('a card needs title or text')
    const text = str(d.text, 'text', 2000)
    return `<div class="pw-card">${d.title ? `<b>${linked(str(d.title, 'title', 120), d.link)}</b>` : ''}${text ? `<p>${esc(text).replace(/\n/g, '<br>')}</p>` : ''}</div>${foot()}`
  }
  if (kind === 'table') return pluginTableHtml(d, size, opened, { str, outLink }) + foot()
  throw new Error(`Echo doesn't draw the kind ${kind}`)
}

/** A table widget's body: its columns (text, number, pill, date; `hide: "M"` left out below L), its rows under their
 *  group's header in the groups' order (the rest last), a row's link on its first text cell, its details under it when
 *  `opened` names it. Throws with what is wrong. */
function pluginTableHtml(d, size, opened, { str, outLink }) {
  const TONES = ['good', 'warn', 'bad', 'muted']
  const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
  if (!Array.isArray(d.columns) || !d.columns.length || d.columns.length > 10) throw new Error('a table needs 1–10 columns: [{key, label, kind?}]')
  const keys = new Set()
  const all = d.columns.map((c, i) => {
    const key = c && typeof c.key === 'string' ? c.key : ''
    if (!/^[A-Za-z0-9_-]{1,40}$/.test(key) || keys.has(key)) throw new Error(`column ${i + 1} needs a plain, unique key`)
    keys.add(key)
    const k = c.kind == null ? 'text' : c.kind
    if (!['text', 'number', 'pill', 'date'].includes(k)) throw new Error(`column ${key}: kind is text, number, pill or date`)
    if (c.hide != null && c.hide !== 'M') throw new Error(`column ${key}: hide is "M"`)
    return { key, label: str(c.label, `column ${key}'s label`, 40), kind: k, hide: c.hide === 'M' }
  })
  const cols = all.filter((c) => !(c.hide && size !== 'L'))
  if (!Array.isArray(d.rows)) throw new Error('a table needs rows: [{id, cells}]')
  if (d.rows.length > 200) throw new Error('a table has 200 rows at most')
  const groups = d.groups == null ? [] : d.groups
  if (!Array.isArray(groups) || groups.length > 50) throw new Error('groups are a list of 50 at most: [{id, label}]')
  const gids = new Map()
  groups.forEach((g, i) => {
    const id = g && (typeof g.id === 'string' || typeof g.id === 'number') ? String(g.id) : ''
    if (!id || gids.has(id)) throw new Error(`group ${i + 1} needs a unique id`)
    gids.set(id, str(g.label, `group ${id}'s label`, 120))
  })
  // A cell as drawn: by its column's kind; {text, tone} anywhere.
  const cell = (v, col, row) => {
    const what = `row ${row}'s cell “${col.key}”`
    const obj = v && typeof v === 'object'
    if (obj && !Array.isArray(v) && v.tone != null && !TONES.includes(v.tone)) throw new Error(`${what}: tone is good, warn, bad or muted`)
    const raw = obj && !Array.isArray(v) ? v.text : v
    const t = obj && !Array.isArray(v) && v.tone ? ` tone-${v.tone}` : ''
    if (raw == null || raw === '') return { html: '', text: '', t }
    if (col.kind === 'number') {
      if (!Number.isFinite(typeof raw === 'number' ? raw : Number.NaN)) throw new Error(`${what} isn't a number`)
      return { html: esc(String(raw)), text: String(raw), t }
    }
    const text = str(raw, what)
    if (col.kind === 'date') {
      const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(text)
      if (!m || +m[2] < 1 || +m[2] > 12 || +m[3] < 1 || +m[3] > 31) throw new Error(`${what} isn't a date (YYYY-MM-DD)`)
      return { html: `<time datetime="${esc(text)}">${MON[+m[2] - 1]} ${+m[3]}</time>`, text, t }
    }
    if (col.kind === 'pill') return { html: `<span class="pw-pill${t}">${esc(text)}</span>`, text, t: '' }
    return { html: esc(text), text, t }
  }
  const ids = new Set()
  const rows = d.rows.map((r, i) => {
    if (!r || typeof r !== 'object' || Array.isArray(r)) throw new Error(`row ${i + 1} is not an object`)
    const id = typeof r.id === 'string' || typeof r.id === 'number' ? String(r.id) : ''
    if (!id || id.length > 80 || ids.has(id)) throw new Error(`row ${i + 1} needs a unique id`)
    ids.add(id)
    if (r.group != null && !gids.has(String(r.group))) throw new Error(`row ${i + 1}'s group “${String(r.group).slice(0, 40)}” isn't in groups`)
    if (r.cells != null && (typeof r.cells !== 'object' || Array.isArray(r.cells))) throw new Error(`row ${i + 1}'s cells are {key: value}`)
    const cells = r.cells || {}
    const drawn = all.map((c) => [c, cell(cells[c.key], c, i + 1)]).filter(([c]) => cols.includes(c)).map(([, x]) => x)
    if (r.detail != null && (!Array.isArray(r.detail) || r.detail.length > 6)) throw new Error(`row ${i + 1}'s detail is a list of 6 lines at most`)
    const detail = (r.detail || []).map((x, j) => ({ text: str(x && x.text, `row ${i + 1}'s detail ${j + 1}`), sub: str(x && x.sub, `row ${i + 1}'s detail ${j + 1}`) }))
    return { id, group: r.group == null ? null : String(r.group), link: r.link, drawn, detail }
  })
  if (!rows.length) return `<p class="set-sub">${esc(str(d.empty, 'empty') || 'Nothing here.')}</p>`
  const details = rows.some((r) => r.detail.length)
  const linkAt = cols.findIndex((c) => c.kind === 'text')
  const open = new Set((opened || []).map(String))
  const span = cols.length + (details ? 1 : 0)
  const line = (r) => {
    const shown = open.has(r.id) && r.detail.length > 0
    const chev = !details ? '' : r.detail.length
      ? `<td class="pw-tc"><button type="button" class="pw-chev" data-pw-row="${esc(r.id)}" aria-expanded="${shown}" aria-label="${shown ? 'Hide details' : 'Show details'}">${shown ? '▾' : '▸'}</button></td>` : '<td class="pw-tc"></td>'
    const tds = r.drawn.map((x, j) => {
      const html = (j === linkAt && x.text && outLink(x.text, r.link)) || x.html
      // A text cell keeps to one line, cut with … when narrow; its whole text on hover.
      return `<td class="k-${cols[j].kind}${x.t}"${cols[j].kind === 'text' && x.text ? ` title="${esc(x.text)}"` : ''}>${html}</td>`
    }).join('')
    const more = shown ? `<tr class="pw-td"><td colspan="${span}"><div>${r.detail.map((x) => `<span>${esc(x.text)}</span><span class="set-sub">${esc(x.sub)}</span>`).join('')}</div></td></tr>` : ''
    return `<tr>${chev}${tds}</tr>${more}`
  }
  const body = []
  for (const [gid, label] of gids) {
    const in_ = rows.filter((r) => r.group === gid)
    if (in_.length) body.push(`<tr class="pw-tg"><th colspan="${span}" scope="rowgroup">${esc(label)} <span class="mono">${in_.length}</span></th></tr>`, ...in_.map(line))
  }
  const rest = rows.filter((r) => r.group == null)
  if (rest.length && gids.size) body.push(`<tr class="pw-tg"><th colspan="${span}" scope="rowgroup">Other <span class="mono">${rest.length}</span></th></tr>`)
  body.push(...rest.map(line))
  const head = `${details ? '<th class="pw-tc" aria-label="Details"></th>' : ''}${cols.map((c) => `<th scope="col" class="k-${c.kind}">${esc(c.label)}</th>`).join('')}`
  return `<div class="pw-table"><table><thead><tr>${head}</tr></thead><tbody>${body.join('')}</tbody></table></div>`
}

/** A widget its plugin never answered and whose ask failed (no data yet, not being asked): why, in its frame — it
 *  goes once the plugin answers. Else '' (answered with nothing, or cleared: no place). */
function pluginWidgetFailed(w) {
  return w && w.empty && w.error && !w.at && !w.pending ? `<p class="set-sub warn">Couldn't get its data — ${esc(w.error)}</p>` : ''
}
/** A plugin's chart (`kind: chart`; Workbench `WidgetsCharts`): Echo draws it as SVG in its own palette — line, area,
 *  bar (grouped), stacked, bars and stacked-bars (horizontal), pie, donut, funnel; one series (`points: [{label, value}]`)
 *  or several (`series` + each point's `values`); the answer above it (`headline`); a legend for 2+ series or slices
 *  (none at S); every mark titled with its value. Throws with what is wrong in the data. */
const CHART_TYPES = ['line', 'area', 'bar', 'bars', 'stacked', 'stacked-bars', 'pie', 'donut', 'funnel']
function pluginChartHtml(d, size, { str, num, foot }) {
  const type = d.type
  if (!CHART_TYPES.includes(type)) throw new Error(`a chart needs type: ${CHART_TYPES.join(', ')}`)
  if (!Array.isArray(d.points) || !d.points.length) throw new Error('a chart needs points: [{label, value}]')
  const PIE = type === 'pie' || type === 'donut', ONE = PIE || type === 'funnel', pct100 = d.percent === true
  const unit = d.unit ? ` ${str(d.unit, 'unit', 12)}` : ''
  const MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
  // A date as Echo writes it (Nov 8); any other label as given.
  const lab = (v, what) => { const t = str(v, what, 40), m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(t); return m && +m[2] >= 1 && +m[2] <= 12 ? `${MON[+m[2] - 1]} ${+m[3]}` : t }
  // Echo's colours, never the plugin's: the palette in order; a `tone` gives one a meaning.
  const TONE = { good: 'var(--me)', warn: 'var(--warn)', bad: 'var(--danger)', muted: 'var(--text-3)' }
  const multi = d.series != null
  if (multi && (!Array.isArray(d.series) || !d.series.length)) throw new Error('series needs [{key, label}]')
  if (multi && d.series.length > 6) throw new Error('a chart takes 6 series at most')
  if (multi && ONE && d.series.length > 1) throw new Error(`a ${type} takes one series`)
  const series = multi
    ? d.series.map((x, i) => {
      if (!x || typeof x.key !== 'string' || !x.key) throw new Error(`series ${i + 1} needs a key`)
      return { key: x.key, label: str(x.label, `series ${i + 1}'s label`, 40) || x.key, color: (typeof x.tone === 'string' && Object.hasOwn(TONE, x.tone) && TONE[x.tone]) || `var(--chart-${i + 1})` }
    })
    : [{ key: '', label: '', color: 'var(--chart-1)' }]
  if (new Set(series.map((x) => x.key)).size !== series.length) throw new Error('series keys must be unique')
  // The points that fit: a line or area its last 60, the bar types their last 24, a funnel its first 8 stages; a pie
  // or donut reads all its slices (6 drawn, the rest in Other).
  const kept = type === 'funnel' ? d.points.slice(0, 8) : PIE ? d.points : d.points.slice(-(type === 'line' || type === 'area' ? 60 : 24))
  const pts = kept.map((p, i) => {
    if (!p || typeof p !== 'object') throw new Error(`point ${i + 1} is not an object`)
    if (multi && (!p.values || typeof p.values !== 'object')) throw new Error(`point ${i + 1} needs values: {<key>: number}`)
    const vals = series.map((x) => {
      const v = multi ? (Object.hasOwn(p.values, x.key) ? p.values[x.key] : null) : p.value
      return multi && v == null ? 0 : num(v, multi ? `point ${i + 1}'s ${x.key}` : `point ${i + 1}'s value`)
    })
    return { label: lab(p.label, `point ${i + 1}'s label`), vals }
  })
  if (pts.some((p) => p.vals.some((v) => Math.abs(v) > 1e15))) throw new Error('a chart takes values up to 10^15')
  if (type !== 'line' && type !== 'bar' && pts.some((p) => p.vals.some((v) => v < 0))) throw new Error(`a ${type} takes no negative values (only line and bar do)`)
  const fmt = (v) => `${Number.isInteger(v) ? v : Math.round(v * 100) / 100}${unit}`
  const say = (p) => (multi ? `${p.label}: ${series.map((x, k) => `${x.label} ${fmt(p.vals[k])}`).join(', ')}` : `${p.label}: ${fmt(p.vals[0])}`)
  // The answer first: its value, what it is, its change (an arrow by `by`'s sign; green when good, red when not).
  let head = ''
  if (d.headline != null) {
    const h = d.headline
    if (typeof h !== 'object' || h.value == null) throw new Error('headline needs value')
    let ch = ''
    if (h.change != null) {
      const by = num(h.change.by, 'headline.change.by')
      if (Math.abs(by) > 1e15) throw new Error('headline.change.by is over 10^15')
      const cls = h.change.good === true ? ' good' : h.change.good === false ? ' bad' : ''
      ch = `<span class="pc-ch${cls}">${by > 0 ? '↑' : by < 0 ? '↓' : '→'} ${esc(fmt(Math.abs(by)))}${h.change.than ? ` vs ${esc(str(h.change.than, 'headline.change.than', 40))}` : ''}</span>`
    }
    head = `<div class="pc-head"><b>${esc(str(h.value, 'headline.value', 24))}</b>${h.label ? `<span class="set-sub">${esc(str(h.label, 'headline.label', 60))}</span>` : ''}${ch}</div>`
  }
  const legend = (items) => (items.length > 1 && size !== 'S' ? `<div class="pc-lg">${items.map(([l, c, v]) => `<span><i style="background:${c}"></i>${esc(l)}${v != null ? `<b>${esc(v)}</b>` : ''}</span>`).join('')}</div>` : '')
  const note = d.note && size !== 'S' ? `<div class="set-sub pw-note">${esc(str(d.note, 'note'))}</div>` : ''
  const all = pts.map(say).join('; ')
  let body
  if (PIE) {
    // Slices in order; those under 3%, and past the 6th, fold into Other.
    const sum0 = pts.reduce((a, p) => a + p.vals[0], 0)
    if (!sum0) return `${head}<p class="set-sub pw-note">Nothing to show yet.</p>${note}${foot()}`
    const total = sum0
    let slices = pts.map((p, i) => ({ label: p.label, v: p.vals[0], color: `var(--chart-${i + 1})` }))
    const big = slices.filter((x) => x.v / total >= 0.03).slice(0, 6), other = total - big.reduce((a, x) => a + x.v, 0)
    slices = big.map((x, i) => ({ ...x, color: `var(--chart-${i + 1})` }))
    if (other > 0) slices.push({ label: 'Other', v: other, color: 'var(--text-3)' })
    const R = 56, r = type === 'donut' ? 38 : 0, C = 60
    const at = (a, rad) => `${(C + rad * Math.sin(a)).toFixed(1)},${(C - rad * Math.cos(a)).toFixed(1)}`
    let a0 = 0
    const arcs = slices.map((x) => {
      const a1 = a0 + (x.v / total) * Math.PI * 2, big1 = a1 - a0 > Math.PI ? 1 : 0
      const full = x.v >= total
      const path = full ? (r ? `M${C},${C - R} A${R},${R} 0 1 1 ${C - 0.01},${C - R} L${C - 0.01},${C - r} A${r},${r} 0 1 0 ${C},${C - r} Z` : `M${C},${C - R} A${R},${R} 0 1 1 ${C - 0.01},${C - R} Z`)
        : r ? `M${at(a0, R)} A${R},${R} 0 ${big1} 1 ${at(a1, R)} L${at(a1, r)} A${r},${r} 0 ${big1} 0 ${at(a0, r)} Z` : `M${C},${C} L${at(a0, R)} A${R},${R} 0 ${big1} 1 ${at(a1, R)} Z`
      a0 = a1
      return `<path d="${path}" style="fill:${x.color}"><title>${esc(`${x.label}: ${fmt(x.v)}`)}</title></path>`
    }).join('')
    const center = type === 'donut' && d.center && d.center.value != null ? `<div class="pc-center"><b>${esc(str(d.center.value, 'center.value', 12))}</b>${d.center.label ? `<span>${esc(str(d.center.label, 'center.label', 24))}</span>` : ''}</div>` : ''
    const pct = (v) => `${Math.round((v / total) * 100)}%`
    body = `<div class="pc-pie"><div class="pc-disc"><svg viewBox="0 0 120 120" role="img" aria-label="${esc(slices.map((x) => `${x.label} ${pct(x.v)}`).join(', '))}">${arcs}</svg>${center}</div>${size === 'S' || slices.length < 2 ? '' : `<div class="pc-lg col">${slices.map((x) => `<span><i style="background:${x.color}"></i>${esc(x.label)}<em class="pc-share" aria-hidden="true"><span style="width:${((x.v / total) * 100).toFixed(1)}%;background:${x.color}"></span></em><b>${esc(type === 'pie' ? pct(x.v) : fmt(x.v))}</b></span>`).join('')}</div>`}</div>`
  } else if (type === 'funnel') {
    const st = pts, first = Math.max(...st.map((p) => p.vals[0])) || 1
    body = `<div class="pc-rows funnel" role="img" aria-label="${esc(all)}">${st.map((p, i) => {
      const prev = i ? st[i - 1].vals[0] : null
      return `<span class="pc-rl">${esc(p.label)}</span><span class="pc-bar"><span style="width:${((p.vals[0] / first) * 100).toFixed(1)}%;background:var(--chart-1)" title="${esc(say(p))}"></span></span><span class="mono pc-rn">${esc(fmt(p.vals[0]))}</span><span class="mono pc-rn dim">${prev ? `${Math.round((p.vals[0] / prev) * 100)}%` : ''}</span>`
    }).join('')}</div>`
  } else if (type === 'bars' || type === 'stacked-bars') {
    const rows = pts.slice(-(size === 'L' ? 12 : size === 'S' ? 4 : 6))
    const sum = (p) => p.vals.reduce((a, v) => a + v, 0)
    const max = type === 'stacked-bars' ? (pct100 ? 1 : Math.max(...rows.map(sum)) || 1) : Math.max(...rows.flatMap((p) => p.vals)) || 1
    const cell = (p) => {
      if (type === 'bars') return series.map((x, k) => `<span class="pc-bar"><span style="width:${((p.vals[k] / max) * 100).toFixed(1)}%;background:${x.color}" title="${esc(`${p.label}${multi ? ` · ${x.label}` : ''}: ${fmt(p.vals[k])}`)}"></span></span>`).join('')
      const t = sum(p) || 1
      return `<span class="pc-bar seg">${series.map((x, k) => `<span style="width:${(((pct100 ? p.vals[k] / t : p.vals[k] / max)) * 100).toFixed(1)}%;background:${x.color}" title="${esc(`${p.label} · ${x.label}: ${pct100 ? `${Math.round((p.vals[k] / t) * 100)}%` : fmt(p.vals[k])}`)}"></span>`).join('')}</span>`
    }
    const end = (p) => (type === 'stacked-bars' ? (pct100 ? `${Math.round((p.vals[0] / (sum(p) || 1)) * 100)}%` : fmt(sum(p))) : multi ? '' : fmt(p.vals[0]))
    body = `<div class="pc-rows" role="img" aria-label="${esc(all)}">${rows.map((p) => `<span class="pc-rl">${esc(p.label)}</span><span class="pc-cell">${cell(p)}</span><span class="mono pc-rn">${esc(end(p))}</span>`).join('')}</div>${legend(series.map((x) => [x.label, x.color]))}`
  } else {
    // Line, area, bar (grouped), stacked: one y-axis from 0 (below it when a value is negative).
    const W = 480, H = size === 'S' ? 72 : 120, n = pts.length
    const sums = pts.map((p) => p.vals.reduce((a, v) => a + v, 0))
    const flat = pts.flatMap((p) => p.vals)
    const hi = type === 'stacked' ? (pct100 ? 1 : Math.max(...sums, 0)) : Math.max(...flat, 0)
    const lo = type === 'stacked' ? 0 : Math.min(...flat, 0)
    const span = hi - lo || 1, y = (v) => (H - 2 - ((v - lo) / span) * (H - 10))
    const grid = [0.25, 0.5, 0.75, 1].map((f) => `<line x1="0" x2="${W}" y1="${(H - 2 - f * (H - 10)).toFixed(1)}" y2="${(H - 2 - f * (H - 10)).toFixed(1)}" class="pc-grid"/>`).join('') + `<line x1="0" x2="${W}" y1="${y(0).toFixed(1)}" y2="${y(0).toFixed(1)}" class="pc-base"/>`
    const xAt = (i) => (n === 1 ? W / 2 : (i * W) / (n - 1))
    let marks = ''
    if (type === 'line' || type === 'area') {
      marks = series.map((x, k) => {
        const line = pts.map((p, i) => `${xAt(i).toFixed(1)},${y(p.vals[k]).toFixed(1)}`).join(' ')
        const fill = type === 'area' ? `<polygon points="0,${y(0).toFixed(1)} ${line} ${W},${y(0).toFixed(1)}" style="fill:${x.color}" class="pc-area"/>` : ''
        // Dots round however wide the chart is drawn (a zero-length line with a round cap); unseen past 30 points, still hovered.
        const dots = pts.map((p, i) => { const cx = xAt(i).toFixed(1), cy = y(p.vals[k]).toFixed(1); return `<line class="pc-dot" x1="${cx}" y1="${cy}" x2="${cx}" y2="${cy}" style="stroke:${x.color}${n > 30 ? ';opacity:0' : ''}" vector-effect="non-scaling-stroke"><title>${esc(`${p.label}${multi ? ` · ${x.label}` : ''}: ${fmt(p.vals[k])}`)}</title></line>` }).join('')
        return `${fill}<polyline points="${line}" fill="none" style="stroke:${x.color}" vector-effect="non-scaling-stroke"/>${dots}`
      }).join('')
    } else {
      const step = W / n, inner = step * 0.7, bw = type === 'stacked' ? inner : inner / series.length
      marks = pts.map((p, i) => {
        const x0 = i * step + step * 0.15
        if (type === 'stacked') {
          const t = sums[i] || 1
          let acc = 0
          return series.map((x, k) => {
            const v = pct100 ? p.vals[k] / t : p.vals[k]
            const top = y(acc + v), bot = y(acc)
            acc += v
            const hgt = Math.max(0, bot - top - 2)
            return hgt > 0 ? `<rect x="${x0.toFixed(1)}" y="${(top + 1).toFixed(1)}" width="${inner.toFixed(1)}" height="${hgt.toFixed(1)}" style="fill:${x.color}"><title>${esc(`${p.label} · ${x.label}: ${pct100 ? `${Math.round(v * 100)}%` : fmt(p.vals[k])}`)}</title></rect>` : ''
          }).join('')
        }
        return series.map((x, k) => {
          const v = p.vals[k], top = Math.min(y(v), y(0)), hgt = Math.abs(y(v) - y(0))
          return `<rect x="${(x0 + k * bw + (series.length > 1 ? 1 : 0)).toFixed(1)}" y="${top.toFixed(1)}" width="${Math.max(1, bw - (series.length > 1 ? 2 : 0)).toFixed(1)}" height="${hgt.toFixed(1)}" rx="2" style="fill:${x.color}"><title>${esc(`${p.label}${multi ? ` · ${x.label}` : ''}: ${fmt(v)}`)}</title></rect>`
        }).join('')
      }).join('')
    }
    // Up to 6 dates or names under it, evenly.
    const at = n <= 6 ? pts.map((_, i) => i) : [0, 1, 2, 3, 4, 5].map((k) => Math.round((k * (n - 1)) / 5))
    // A line's labels at its ends and between; a bar's under its bar (each at its slot's middle).
    const ticks = type === 'line' || type === 'area'
      ? `<div class="pc-x mono">${at.map((i) => `<span>${esc(pts[i].label)}</span>`).join('')}</div>`
      : `<div class="pc-x mono pc-under">${at.map((i, k) => {
        // Under its bar, no wider than its bar's slot (so never past the card); with more bars than labels, the
        // first from its bar's left edge and the last to its bar's right edge.
        const sampled = at.length < n, c = at.map((j) => ((j + 0.5) / n) * 100)
        // As wide as the room to its neighbours allows: centred ones half a gap each side, an anchored edge one
        // its whole width on one side (so none runs into the next).
        const gaps = c.slice(1).map((x, j) => x - c[j])
        let room = Math.min(40, ...(sampled ? gaps : [100 / n]))
        if (sampled && c.length > 2) room = Math.min(room, ((c[1] - (at[0] / n) * 100) * 2) / 3, ((((at[at.length - 1] + 1) / n) * 100 - c[c.length - 2]) * 2) / 3)
        const w = `max-width:${room.toFixed(1)}%`
        if (sampled && k === 0) return `<span class="first" style="left:${((i / n) * 100).toFixed(2)}%;${w}">${esc(pts[i].label)}</span>`
        if (sampled && k === at.length - 1) return `<span class="last" style="right:${((1 - (i + 1) / n) * 100).toFixed(2)}%;${w}">${esc(pts[i].label)}</span>`
        return `<span style="left:${(((i + 0.5) / n) * 100).toFixed(2)}%;${w}">${esc(pts[i].label)}</span>`
      }).join('')}</div>`
    body = `<svg class="pc-svg" viewBox="0 0 ${W} ${H}" preserveAspectRatio="none" style="height:${H}px" role="img" aria-label="${esc(all)}">${grid}${marks}</svg>${ticks}${legend(series.map((x) => [x.label, x.color]))}`
  }
  return `${head}${body}${note}${foot()}`
}
// end of the plugin widget block

/** A plugin's widget on Today: its label, then its kind drawn from its data; nothing yet (or cleared): nothing drawn;
 *  data Echo can't draw: "Couldn't draw this" with why; when its provider failed, its last data, dated — or, never
 *  answered, "Couldn't get its data" with why. */
function pluginWidgetView(id, w, size) {
  // Not answered yet, or its page's window half still loading: its place held, a placeholder in it (nothing jumps
  // when it comes). Answered with nothing: no place.
  const plugin = id.split(':')[1]
  const waiting = w && (w.pending || (w.kind === 'page' && !w.empty && !Plugins.widgetDraw[id] && !Plugins.applied[plugin] && !Plugins.errors[plugin]))
  if (waiting) return `<section class="td-sec pw-sec" aria-label="${esc(w.label)}" aria-busy="true" data-pw="${esc(id)}"><h2>${esc(w.label)}</h2><div class="skeleton pw-wait k-${esc(w.kind)} s-${esc(size)}"><i></i><i></i><i></i></div></section>`
  // Never answered and its ask failed: its frame and label, and why (#413's follow-up, the user's decision).
  const failed = pluginWidgetFailed(w)
  if (failed) return `<section class="td-sec pw-sec" aria-label="${esc(w.label)}" data-pw="${esc(id)}"><h2>${esc(w.label)}</h2>${failed}</section>`
  if (!w || w.empty) return ''
  let body
  try {
    if (w.kind === 'page') {
      // Its own page: drawn by its client half (`ctx.widgets.draw`), in Echo's frame.
      const draw = Plugins.widgetDraw[id]
      if (!draw) throw new Error('its page isn\'t loaded (the plugin\'s window half)')
      const theme = document.documentElement.dataset.theme || (matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light')
      Plugins.drawing = true
      try { body = `<div class="pw-page">${String(draw({ data: w.data, size, theme }) ?? '')}</div>` } finally { Plugins.drawing = false }
    } else body = pluginWidgetHtml(w.kind, w.data, size, (S.pwRows && S.pwRows[id]) || [])
  } catch (e) { body = `<p class="set-sub warn">Couldn't draw this — ${esc(e.message)}</p>` }
  // When its data came: plugins answer on their own schedule, so the time says how fresh it is (a day's other date
  // named); after a failed ask, since when it hasn't changed, and why.
  // From the plugin's own earlier read (`asOf` in its data: a timer's fetch, a cache) its data is that old, not as new as
  // its answer; a time ahead of now is not believed.
  const asOf = w.data && typeof w.data.asOf === 'string' ? new Date(w.data.asOf) : null
  const at = asOf && !Number.isNaN(asOf.getTime()) && asOf.getTime() <= Date.now() + 60000 ? asOf : w.at ? new Date(w.at) : null
  const when = at && (at.toDateString() === new Date().toDateString() ? hm(at) : `${at.toLocaleDateString([], { month: 'short', day: 'numeric' })}, ${hm(at)}`)
  // An icon and the time; the words in its label and tooltip.
  const REFRESHED = '<path d="M20 11a8 8 0 0 0-14.3-4.9L4 8"/><path d="M4 4v4h4"/><path d="M4 13a8 8 0 0 0 14.3 4.9L20 16"/><path d="M20 20v-4h-4"/>'
  const upd = (cls, say) => `<span class="pw-upd${cls}" role="img" aria-label="${esc(say)}" title="${esc(say)}">${ICON(REFRESHED)}${esc(when)}</span>`
  const stale = !when ? '' : w.error ? upd(' warn', `Not updated since ${when} — ${w.error}`) : upd('', `Updated ${when}`)
  // A table says how many rows it has, and the plugin's note, beside its label.
  const d = w.data || {}
  const count = w.kind === 'table' && Array.isArray(d.rows) && !body.includes('Couldn\'t draw this') ? ` <span class="mono pw-n">${d.rows.length}</span>${typeof d.note === 'string' && d.note ? `<span class="pw-hnote">· ${esc(d.note.slice(0, 120))}</span>` : ''}` : ''
  // An outside link asked for here: its host said before it opens (a second click on Open) — the one question on the
  // page, a work row's or a widget's (`linkAsked`; it lapses after 30 s).
  const armed = linkAsked(S, 'wrOpen', Date.now())
  // A table row's or a stats item's link has no "Open" to name: the click is on the link itself.
  const again = w.kind === 'table' || w.kind === 'stats' ? 'Click it again' : 'Click Open again'
  const ask = armed && armed.where === `pw:${id}` && body.includes(`data-pw-open="${esc(armed.url)}"`) ? linkAskLine(armed.url, again, 'data-pw-open-no="1"') : ''
  return `<section class="td-sec pw-sec k-${esc(w.kind)}" aria-label="${esc(w.label)}" data-pw="${esc(id)}"><h2>${esc(w.label)}${count} ${stale}</h2>${body}${ask}</section>`
}

// A plugin widget's actions and its outside link: an action runs its plugin's command (the user's click), then Today is
// read again; an https link asks once in the widget ("Open <host>?") before it leaves Echo.
document.addEventListener('click', async (e) => {
  if (S.view !== 'today' || !e.target.closest) return
  const sec = e.target.closest('[data-pw]')
  if (!sec) return
  const plugin = (sec.dataset.pw.split(':')[1] || '')
  // A table row's details: shown or hidden on its ▸ (kept while Today redraws).
  const row = e.target.closest('[data-pw-row]')
  if (row) {
    e.preventDefault()
    const open = new Set(((S.pwRows || {})[sec.dataset.pw]) || [])
    if (open.has(row.dataset.pwRow)) open.delete(row.dataset.pwRow)
    else open.add(row.dataset.pwRow)
    S.pwRows = { ...(S.pwRows || {}), [sec.dataset.pw]: [...open] }
    render()
    setTimeout(() => { const b = document.querySelector(`[data-pw="${CSS.escape(sec.dataset.pw)}"] [data-pw-row="${CSS.escape(row.dataset.pwRow)}"]`); if (b) b.focus() })
    return
  }
  const act = e.target.closest('[data-pw-cmd]')
  if (act) {
    e.preventDefault()
    act.disabled = true
    S.wrOpen = null
    let args = {}
    try { args = JSON.parse(act.dataset.pwArgs || '{}') } catch { /* none */ }
    // The call answers {value} or {error}: a refused or failing command says so.
    try {
      const r = await get(`api/plugin/${encodeURIComponent(plugin)}/call/${encodeURIComponent(act.dataset.pwCmd)}?args=${encodeURIComponent(JSON.stringify(args))}`)
      if (r && r.error) window.echo.notice(`${act.textContent}: ${r.error}`)
    } catch (x) { window.echo.notice(`${act.textContent}: ${x.message || x}`) }
    return loadToday().then(render)
  }
  const open = e.target.closest('[data-pw-open]')
  if (open) {
    e.preventDefault()
    const url = open.dataset.pwOpen
    // The keyboard stays on this widget's Open (the line redrawn).
    const back = () => setTimeout(() => { const b = document.querySelector(`[data-pw="${CSS.escape(sec.dataset.pw)}"] [data-pw-open]`); if (b) b.focus() })
    if (linkAskStep(S, 'wrOpen', url, Date.now(), `pw:${sec.dataset.pw}`)) send({ cmd: 'open_url', url })
    render()
    return back()
  }
  if (e.target.closest('[data-pw-open-no]')) {
    S.wrOpen = null
    render()
    setTimeout(() => { const b = document.querySelector(`[data-pw="${CSS.escape(sec.dataset.pw)}"] [data-pw-open]`); if (b) b.focus() })
  }
})
