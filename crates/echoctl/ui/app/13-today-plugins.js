// ── Plugins' widgets on Today (`plugin:<plugin>:<widget>`, #315; design: Workbench `WidgetsPluginAPI`,
// `WidgetsPluginKinds`): Echo draws a plugin's data by the widget's kind — its text is data, escaped; a link only to
// echo:// (Echo opens it). The data comes from api/today's `plugins` (the last answer of the plugin's host half).

// ── plugin widget: a kind's data as HTML, or why it can't be drawn (no DOM: `today-plugin-widgets.test.mjs`) ───────
/** `kind` (stat, stats, chart, list, status, card) with `data` at `size` → HTML; throws with what is wrong in the data. */
function pluginWidgetHtml(kind, data, size) {
  const d = data && typeof data === 'object' ? data : null
  if (!d) throw new Error('its data is not an object')
  const str = (v, what, max = 200) => { if (v == null) return ''; if (typeof v !== 'string' && typeof v !== 'number') throw new Error(`${what} is not text`); return String(v).slice(0, max) }
  const TONES = ['good', 'warn', 'bad']
  const tone = (t) => (TONES.includes(t) ? ` tone-${t}` : '')
  // A link only to Echo's own places; anything else is shown as text.
  const linked = (text, link) => (typeof link === 'string' && /^echo:\/\/[a-z]+\//.test(link) ? `<a class="link-btn" href="${esc(link)}">${esc(text)}</a>` : esc(text))
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
  if (kind === 'chart') {
    if (!['bar', 'bars', 'line'].includes(d.type)) throw new Error('a chart needs type: bar, bars or line')
    if (!Array.isArray(d.points) || !d.points.length) throw new Error('a chart needs points: [{label, value}]')
    const pts = d.points.slice(-(d.type === 'bars' ? (size === 'L' ? 10 : 6) : 31)).map((p, i) => ({ label: str(p && p.label, `point ${i + 1}'s label`, 40), value: num(p && p.value, `point ${i + 1}'s value`) }))
    const max = Math.max(...pts.map((p) => p.value), 0) || 1, unit = d.unit ? ` ${str(d.unit, 'unit', 12)}` : ''
    const say = (p) => `${p.label}: ${p.value}${unit}`
    let body
    if (d.type === 'bars') {
      body = `<div class="pw-bars">${pts.map((p) => `<span class="pw-bl">${esc(p.label)}</span><span class="pw-bt"><span style="width:${((Math.max(0, p.value) / max) * 100).toFixed(1)}%"></span></span><span class="mono pw-bn">${esc(String(p.value))}</span>`).join('')}</div>`
    } else {
      const w = 480, h = 100, n = pts.length, step = w / n
      const y = (v) => (h - (Math.max(0, v) / max) * (h - 6)).toFixed(1)
      const marks = d.type === 'bar'
        ? pts.map((p, i) => `<rect x="${(i * step + step * 0.15).toFixed(1)}" y="${y(p.value)}" width="${(step * 0.7).toFixed(1)}" height="${(h - y(p.value)).toFixed(1)}" rx="2"${i === n - 1 ? ' class="last"' : ''}><title>${esc(say(p))}</title></rect>`).join('')
        : `<polyline points="${pts.map((p, i) => `${(n === 1 ? w / 2 : (i * w) / (n - 1)).toFixed(1)},${y(p.value)}`).join(' ')}" fill="none" vector-effect="non-scaling-stroke"/>${pts.map((p, i) => `<circle cx="${(n === 1 ? w / 2 : (i * w) / (n - 1)).toFixed(1)}" cy="${y(p.value)}" r="2.5"><title>${esc(say(p))}</title></circle>`).join('')}`
      body = `<svg class="pw-chart ${d.type}" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" role="img" aria-label="${esc(pts.map(say).join(', '))}"><line x1="0" y1="${h}" x2="${w}" y2="${h}"/>${marks}</svg><div class="pw-axis mono"><span>${esc(pts[0].label)}</span><span>${esc(pts[n - 1].label)} · ${esc(String(pts[n - 1].value))}${esc(unit)}</span></div>`
    }
    return `${body}${d.note && size !== 'S' ? `<div class="set-sub pw-note">${esc(str(d.note, 'note'))}</div>` : ''}${foot()}`
  }
  if (kind === 'stats') {
    if (!Array.isArray(d.items)) throw new Error('stats need items: [{label, value}]')
    const items = d.items.slice(0, size === 'L' ? 6 : size === 'S' ? 2 : 4)
    return `<div class="pw-stats">${items.map((x, i) => `<div><b class="${tone(x && x.tone).trim()}">${esc(str(x && x.value, `item ${i + 1}'s value`, 24))}</b><span class="set-sub">${esc(str(x && x.label, `item ${i + 1}'s label`, 60))}</span></div>`).join('')}</div>${foot()}`
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
  throw new Error(`Echo doesn't draw the kind ${kind}`)
}

/** A widget its plugin never answered and whose ask failed (no data yet, not being asked): why, in its frame — it
 *  goes once the plugin answers. Else '' (answered with nothing, or cleared: no place). */
function pluginWidgetFailed(w) {
  return w && w.empty && w.error && !w.at && !w.pending ? `<p class="set-sub warn">Couldn't get its data — ${esc(w.error)}</p>` : ''
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
    } else body = pluginWidgetHtml(w.kind, w.data, size)
  } catch (e) { body = `<p class="set-sub warn">Couldn't draw this — ${esc(e.message)}</p>` }
  const stale = w.error ? `<span class="set-sub" title="${esc(w.error)}">· from ${esc(hm(new Date(w.at)))}</span>` : ''
  // An outside link asked for here: its host said before it opens (a second click on Open) — the one question on the
  // page, a work row's or a widget's (`linkAsked`; it lapses after 30 s).
  const armed = linkAsked(S, 'wrOpen', Date.now())
  const ask = armed && armed.where === `pw:${id}` && body.includes(`data-pw-open="${esc(armed.url)}"`) ? linkAskLine(armed.url, 'Click Open again', 'data-pw-open-no="1"') : ''
  return `<section class="td-sec pw-sec" aria-label="${esc(w.label)}" data-pw="${esc(id)}"><h2>${esc(w.label)} ${stale}</h2>${body}${ask}</section>`
}

// A plugin widget's actions and its outside link: an action runs its plugin's command (the user's click), then Today is
// read again; an https link asks once in the widget ("Open <host>?") before it leaves Echo.
document.addEventListener('click', async (e) => {
  if (S.view !== 'today' || !e.target.closest) return
  const sec = e.target.closest('[data-pw]')
  if (!sec) return
  const plugin = (sec.dataset.pw.split(':')[1] || '')
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
