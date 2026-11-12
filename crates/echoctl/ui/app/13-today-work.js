// ── plugin work rows: an item a plugin gives (`ctx.work.provide`) on My work and in a thread's Work tab — its plugin's
// icon in place of the thread's ("From <plugin>"), its kind and where, its title opening its https page in the browser
// once the user confirms it (as a plugin widget's link: "Open <host> in the browser?", then a second click); no Close
// or Claude (they are Echo's own tasks'), only its plugins' `task.actions` that ask for it (`acts`). Not updated the
// last time it was asked (`stale`: why): its last rows, marked so.
function workRowIcon(r) {
  const from = `From ${r.source_name || r.source || 'a plugin'}`
  return r.icon ? `<img class="wr-ic" src="${esc(r.icon)}" alt="${esc(from)}" title="${esc(from)}" width="12" height="12">`
    : `<span class="wr-ic wr-letter" title="${esc(from)}" role="img" aria-label="${esc(from)}">${esc(String(r.source_name || r.source || '?').trim().charAt(0).toUpperCase())}</span>`
}
/** An outside page's host, as the question names it. */
const linkHost = (url) => { try { return new URL(url).host } catch { return 'the link' } }
/** How long a question about an outside link stays armed (ms). */
const LINK_ASK_MS = 30000
/** The outside link armed in `st[key]` — `{url, where}` (`where`: the place that asked, a work row or a widget: only it
 *  shows the question) — or null when none is, or it lapsed. Reads only: drawing never changes it. */
function linkAsked(st, key, now) {
  if (!st[key] || now - (st[key + 'At'] || 0) > LINK_ASK_MS) return null
  return { url: st[key], where: st[key + 'Where'] || '' }
}
/** A click on an outside link in a place (`where`): the first arms it (that place asks "Open <host>?"; one question
 *  at a time, any other goes), a second on the same link in the same place within 30 s says to open it now (true)
 *  and disarms it. */
function linkAskStep(st, key, url, now, where = '') {
  const armed = linkAsked(st, key, now)
  if (armed && armed.url === url && armed.where === where) { st[key] = null; return true }
  st[key] = url
  st[key + 'At'] = now
  st[key + 'Where'] = where
  return false
}
/** The question under an armed link: its host, and how to go on or not (Escape too); read out when it appears. */
const linkAskLine = (url, again, no) => `<div class="pw-ask set-sub" role="status" aria-live="polite">Open ${esc(linkHost(url))} in the browser? ${again} · <button type="button" class="link-btn" ${no}>Cancel</button></div>`
/** Where a work row's question shows: that row. */
const workRowWhere = (r) => `row:${r.source}:${r.id}`
/** A plugin's work row; `ask`: the outside link armed now, `linkAsked`'s (its row says so when it is this row's). */
function workRowHtml(r, due, acts, ask) {
  const url = /^https:\/\/[^\s"<>\\]+$/.test(r.url || '') ? r.url : ''
  const where = workRowWhere(r)
  const title = url ? `<button type="button" class="link-btn td-task-title" data-open-url="${esc(url)}" data-open-where="${esc(where)}" title="Open it in the browser (${esc(linkHost(url))})">${esc(r.title)}</button>` : `<span class="td-task-title">${esc(r.title)}</span>`
  const what = [r.kind, r.where].filter(Boolean).map(esc).join(' · ')
  const stale = r.stale ? `<span class="wr-stale" title="${esc(`Not updated: ${r.stale}`)}">not updated</span>` : ''
  const asked = url && ask && ask.url === url && ask.where === where ? linkAskLine(url, 'Click its title again', 'data-open-url-no="1"') : ''
  return `<div class="td-task td-plug" role="group" aria-label="${esc(`From ${r.source_name || r.source}: ${r.title}`)}"><span class="td-task-t">${due || ''}${title}</span><span class="td-task-m">${workRowIcon(r)}<span class="td-task-w" title="${what}">${what}</span>${stale}${acts ? `<span class="td-icos">${acts}</span>` : ''}</span>${asked}</div>`
}
// end of the plugin work rows block
/** A row's `task.actions` (only the plugins' items that say they act on it: `when` true for `{kind: 'plugin', source,
 *  row}`), each row its own target. */
const workRowActs = (r) => slotItems('task.actions', { kind: 'plugin', source: r.source, row: r }, 'buttons', `${r.source}:${r.id}`)
/** A thread's Work tab: the plugins' items of this thread, above its own work (never in its ledger). */
function threadWorkRows(w) {
  const rows = (w && w.plugin_rows) || []
  if (!rows.length) return ''
  const due = (r) => r.late ? '<span class="wr-late">late</span> ' : r.due ? `<span class="set-sub">${esc(shortDay(r.due + 'T12:00'))}</span> ` : ''
  const ask = linkAsked(S, 'wrOpen', Date.now())
  return `<section class="wk-plug" aria-label="From plugins"><div class="td-tasks">${rows.map((r) => workRowHtml(r, due(r), workRowActs(r), ask)).join('')}</div></section>`
}
/** A click on a work row's title (or its ⌘K entry, `press`): asked first, opened on the second; the keyboard stays on
 *  that title (the row redrawn). */
function workRowOpen(url, where = '') {
  if (linkAskStep(S, 'wrOpen', url, Date.now(), where)) send({ cmd: 'open_url', url })
  render()
  setTimeout(() => { const b = document.querySelector(`[data-open-url="${CSS.escape(url)}"][data-open-where="${CSS.escape(where)}"]`); if (b) b.focus() })
}
// Escape takes the question back (a work row's or a widget's), the keyboard left where it was.
document.addEventListener('keydown', (e) => {
  if (e.key !== 'Escape' || e.defaultPrevented || (typeof PAL !== 'undefined' && PAL.open) || !linkAsked(S, 'wrOpen', Date.now())) return
  e.preventDefault()
  S.wrOpen = null
  render()
})
