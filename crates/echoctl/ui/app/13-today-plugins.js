// ── Plugins' widgets on Today (`plugin:<plugin>:<widget>`, #315; design: Workbench `WidgetsPluginAPI`,
// `WidgetsPluginKinds`): Echo draws a plugin's data by the widget's kind — its text is data, escaped; a link only to
// echo:// (Echo opens it). The data comes from api/today's `plugins` (the last answer of the plugin's host half).

// ── plugin widget: a kind's data as HTML, or why it can't be drawn (no DOM: `today-plugin-widgets.test.mjs`) ───────
/** `kind` (stat, stats, list, status, card) with `data` at `size` → HTML; throws with what is wrong in the data. */
function pluginWidgetHtml(kind, data, size) {
  const d = data && typeof data === 'object' ? data : null
  if (!d) throw new Error('its data is not an object')
  const str = (v, what, max = 200) => { if (v == null) return ''; if (typeof v !== 'string' && typeof v !== 'number') throw new Error(`${what} is not text`); return String(v).slice(0, max) }
  const TONES = ['good', 'warn', 'bad']
  const tone = (t) => (TONES.includes(t) ? ` tone-${t}` : '')
  // A link only to Echo's own places; anything else is shown as text.
  const linked = (text, link) => (typeof link === 'string' && /^echo:\/\/[a-z]+\//.test(link) ? `<a class="link-btn" href="${esc(link)}">${esc(text)}</a>` : esc(text))
  if (kind === 'stat') {
    if (d.value == null) throw new Error('a stat needs value')
    return `<div class="pw-stat"><b class="${tone(d.tone).trim()}">${esc(str(d.value, 'value', 24))}</b>${d.label ? `<span class="set-sub">${esc(str(d.label, 'label', 80))}</span>` : ''}</div>${d.note && size !== 'S' ? `<div class="set-sub pw-note">${esc(str(d.note, 'note'))}</div>` : ''}`
  }
  if (kind === 'stats') {
    if (!Array.isArray(d.items)) throw new Error('stats need items: [{label, value}]')
    const items = d.items.slice(0, size === 'L' ? 6 : size === 'S' ? 2 : 4)
    return `<div class="pw-stats">${items.map((x, i) => `<div><b class="${tone(x && x.tone).trim()}">${esc(str(x && x.value, `item ${i + 1}'s value`, 24))}</b><span class="set-sub">${esc(str(x && x.label, `item ${i + 1}'s label`, 60))}</span></div>`).join('')}</div>`
  }
  if (kind === 'list') {
    if (!Array.isArray(d.items)) throw new Error('a list needs items: [{title, sub?, link?}]')
    const items = d.items.slice(0, size === 'L' ? 10 : size === 'S' ? 3 : 5)
    if (!items.length) return `<p class="set-sub">${esc(str(d.empty, 'empty') || 'Nothing here.')}</p>`
    return `<div class="pw-list" role="list">${items.map((x, i) => `<div class="pw-li" role="listitem"><span>${linked(str(x && x.title, `item ${i + 1}'s title`), x && x.link)}</span>${x && x.sub ? `<span class="set-sub">${esc(str(x.sub, `item ${i + 1}'s sub`))}</span>` : ''}</div>`).join('')}</div>`
  }
  if (kind === 'status') {
    if (!['ok', 'warn', 'bad'].includes(d.state)) throw new Error('a status needs state: ok, warn or bad')
    return `<div class="pw-status st-${d.state}"><i aria-hidden="true"></i><b>${esc(str(d.title, 'title', 80))}</b>${d.text ? `<span class="set-sub">${esc(str(d.text, 'text', 300))}</span>` : ''}</div>`
  }
  if (kind === 'card') {
    if (d.title == null && d.text == null) throw new Error('a card needs title or text')
    const text = str(d.text, 'text', 2000)
    return `<div class="pw-card">${d.title ? `<b>${linked(str(d.title, 'title', 120), d.link)}</b>` : ''}${text ? `<p>${esc(text).replace(/\n/g, '<br>')}</p>` : ''}</div>`
  }
  throw new Error(`Echo doesn't draw the kind ${kind}`)
}
// end of the plugin widget block

/** A plugin's widget on Today: its label, then its kind drawn from its data; nothing yet (or cleared): nothing drawn;
 *  data Echo can't draw: "Couldn't draw this" with why; when its provider failed, its last data, dated. */
function pluginWidgetView(id, w, size) {
  if (!w || w.empty) return ''
  let body
  try { body = pluginWidgetHtml(w.kind, w.data, size) } catch (e) { body = `<p class="set-sub warn">Couldn't draw this — ${esc(e.message)}</p>` }
  const stale = w.error ? `<span class="set-sub" title="${esc(w.error)}">· from ${esc(hm(new Date(w.at)))}</span>` : ''
  return `<section class="td-sec pw-sec" aria-label="${esc(w.label)}" data-pw="${esc(id)}"><h2>${esc(w.label)} ${stale}</h2>${body}</section>`
}
