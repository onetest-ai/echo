// ── Plugins on a call's page: their boxes on its header (`call.header`), their lines while its minutes are coming
// (`call.minutes`), a plugin's sheet over the window (`ctx.ui.sheet`), and its activity's badge and editing guard.
// Each draws only the HTML its plugin gives, in Echo's own generic frame.

// ── call page slots: a call's facts for its plugins, their header boxes and minutes lines (no DOM:
// `call-slots.test.mjs`) — each item drawn on its own by `threadSlotRows` (one that throws left out and logged, a second
// with the same plugin and id left out), `after(box, call)` once per box drawn afresh.
/** The call a `call.header` box or a `call.minutes` line is drawn for: what the page already has of it. */
function slotCallPage(c) {
  const ps = c.projects || []
  return {
    name: c.name, stamp: callStamp(c.name) || c.name, title: c.title, started_at: c.started_at || null, notes: !!c.notes,
    profile: (c.live && c.live.profile) || null, memo: !!c.memo,
    threads: ps.map((p) => p.id), client: (ps.find((p) => p.client) || {}).client || null,
  }
}
/** The plugins' boxes on the call's header, each marked `data-call-slot` for its `after`. */
const callHeadBoxes = (items, call, log) => threadSlotRows(items, call, log)
  .map((r) => `<span class="plug-head" data-call-slot="${esc(r.key)}">${r.html}</span>`).join('')
/** Each box's `after(box, call)`, once per box drawn afresh (or drawn for another call). */
function callSlotAfter(root, items, call, log) {
  const boxes = [...root.querySelectorAll('[data-call-slot]')]
  const seen = new Set()
  for (const x of items || []) {
    const key = `${x.plugin}:${x.id}`
    if (seen.has(key)) continue
    seen.add(key)
    const box = x.after && boxes.find((b) => b.dataset.callSlot === key)
    if (!box || box._afterFor === call.name) continue
    box._afterFor = call.name
    try { x.after(box, call) } catch (e) { log(`${x.plugin}: ${x.label || x.id}: ${(e && e.message) || e}`) }
  }
}
/** Whether the call's minutes are on their way: a run queued or going, or Echo writing them by itself (due now or at a
 *  time, the call not over or not transcribed yet, a Claude Code session holding it). Not when they are off, held,
 *  too thin, can't start, or a run failed. */
function minutesComing(c) {
  const run = c.notes_run, auto = c.auto || {}
  if (run && (run.state === 'queued' || run.state === 'running')) return true
  if (c.notes) return false
  if (run && ['failed', 'needs_login', 'skipped'].includes(run.state)) return false
  if (!run && (c.status === 'failed' || c.status === 'audio')) return false
  // `before`: the call ended before automatic minutes were on — Echo won't write them by itself.
  return ['now', 'at', 'not_ready'].includes(auto.due) || (auto.due === 'tried' && !!auto.held_by)
}
/** The plugins' lines in the minutes' box, only while they are coming; a line with nothing to say isn't drawn. */
const callMinutesLines = (items, c, log) => !minutesComing(c) ? '' : threadSlotRows(items, slotCallPage(c), log)
  .filter((r) => r.html.trim())
  .map((r) => `<div class="alt plug-line" data-call-minutes="${esc(r.key)}">${r.html}</div>`).join('')
// end of the call page slots block

// ── plugin sheet: the one plugin sheet open, and where Tab goes in it (no DOM: `call-slots.test.mjs`).
const Sheet = { now: null }
/** The open sheet closed — any (`plugin` null) or only that plugin's — and its `onClose` called once, unless `quiet` (its
 *  plugin unloading). The sheet closed, or null when there was none to close. */
function sheetClose(plugin, quiet, log) {
  const s = Sheet.now
  if (!s || (plugin && s.plugin !== plugin)) return null
  Sheet.now = null
  if (!quiet && typeof s.onClose === 'function') { try { s.onClose() } catch (e) { log(`${s.plugin}: its sheet's onClose: ${(e && e.message) || e}`) } }
  return s
}
/** A sheet opened for `plugin`: the one open before closed first (its onClose called). */
function sheetOpen(plugin, onClose, back, log) {
  const was = sheetClose(null, false, log)
  Sheet.now = was ? { plugin, onClose, back: was.back, backSel: was.backSel } : { plugin, onClose, back }
  return Sheet.now
}
/** The focusable to go to on Tab (`back`: with Shift) from the one at `at` (-1: none of them) of `n`: round, never out. */
const sheetTab = (n, at, back) => !n ? -1 : at < 0 ? (back ? n - 1 : 0) : (at + (back ? n - 1 : 1)) % n
// end of the plugin sheet block

const sheetLog = (m) => console.warn(`plugin sheet: ${m}`)
const SHEET_FOCUS = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
const sheetFocusables = (box) => [...box.querySelectorAll(SHEET_FOCUS)].filter((el) => el.offsetParent !== null || el === document.activeElement)
/** Draw a plugin's sheet in Echo's modal frame, over the window (its own holder beside `#r-over`: Echo's redraws never
 *  replace it), the keyboard moved into it. Returns its box. */
function sheetShow(plugin, html, opts) {
  const old = document.getElementById('plug-sheet')
  // Opened from a menu's item (gone once the menu closes): the keyboard goes back to the menu's button.
  const a = document.activeElement, menu = a && a.closest && a.closest('[role=menu]')
  const btn = menu && menu.previousElementSibling
  const back = btn && btn.getAttribute('aria-haspopup') === 'menu' ? btn : a
  const s = sheetOpen(plugin, opts.onClose, back, sheetLog)
  // Echo may draw that button again meanwhile: found again by its id, command or name.
  if (s.back === back && back && back.getAttribute) {
    const q = (k) => back.getAttribute(k)
    s.backSel = back.id ? `#${CSS.escape(back.id)}` : q('data-cmd') ? `[data-cmd="${CSS.escape(q('data-cmd'))}"]` : q('aria-label') ? `[aria-label="${CSS.escape(q('aria-label'))}"]` : ''
  }
  if (old) old.remove()
  const el = document.createElement('div')
  el.id = 'plug-sheet'; el.className = 'td-modal plug-scrim'; el.dataset.plugin = plugin
  el.innerHTML = `<div class="td-dialog plug-sheet" role="dialog" aria-modal="true" aria-label="${esc(String(opts.label || plugin))}" tabindex="-1"></div>`
  const box = el.firstElementChild
  box.innerHTML = String(html == null ? '' : html)
  const over = document.getElementById('r-over')
  if (over) over.after(el); else document.body.appendChild(el)
  s.el = el
  const first = box.querySelector('[autofocus]') || sheetFocusables(box)[0] || box
  first.focus()
  return box
}
/** The sheet closed (any, or only `plugin`'s; `quiet`: without its onClose) and the keyboard back where it was. */
function sheetShut(plugin, quiet) {
  const s = sheetClose(plugin, quiet, sheetLog)
  if (!s) return false
  if (s.el) s.el.remove()
  const back = s.back && s.back.isConnected ? s.back : s.backSel ? document.querySelector(s.backSel) : null
  if (back && typeof back.focus === 'function') back.focus()
  return true
}
// Esc closes it (the palette over it keeps its own Escape); Tab goes round inside it.
document.addEventListener('keydown', (e) => {
  // The palette or one of Echo's own dialogs over it keeps its own keys.
  const dlg = document.getElementById('r-dlg')
  if (!Sheet.now || !Sheet.now.el || PAL.open || (dlg && dlg.firstElementChild)) return
  if (e.key === 'Escape') { e.preventDefault(); e.stopImmediatePropagation(); sheetShut(null); return }
  if (e.key !== 'Tab') return
  const box = Sheet.now.el.firstElementChild, list = sheetFocusables(box)
  const to = sheetTab(list.length, list.indexOf(document.activeElement), e.shiftKey)
  e.preventDefault()
  ;(to < 0 ? box : list[to]).focus()
}, true)
// Another place opened under it (a shortcut, the tray, `ctx.go`): it closes, its onClose called.
window.addEventListener('hashchange', () => { if (Sheet.now) sheetShut(null) })
// A click beside it (on its backdrop) closes it.
document.addEventListener('click', (e) => { if (Sheet.now && Sheet.now.el && e.target === Sheet.now.el) sheetShut(null) })

// ── plugin activity: a plugin activity's badge on its icon, and whether its box is left as it is while the user edits in
// it (no DOM: `call-slots.test.mjs`). A `badge()` or `editing()` that throws is logged once for that activity, then
// taken as none / not editing.
/** A badge as drawn: a count (1–99, more as "99+") or '!' (it needs the user); anything else, none. */
function actBadgeText(v) {
  if (v === '!') return '!'
  const n = typeof v === 'number' ? v : typeof v === 'string' && /^\s*\d+\s*$/.test(v) ? Number(v) : NaN
  return Number.isFinite(n) && n >= 1 ? (n > 99 ? '99+' : String(Math.floor(n))) : ''
}
/** A plugin activity's `hook` (`badge`, `editing`) asked: its answer, or `fallback` when it has none or throws. A
 *  `ctx.render()` asked for in it is ignored. */
function actAsk(a, hook, fallback, log) {
  if (typeof a[hook] !== 'function') return fallback
  Plugins.drawing = true
  try { return a[hook]() } catch (e) {
    a._failed = a._failed || {}
    if (!a._failed[hook]) { a._failed[hook] = true; log(`${a.plugin}: ${a.label || a.id}: ${hook}: ${(e && e.message) || e}`) }
    return fallback
  } finally { Plugins.drawing = false }
}
/** Echo leaves the activity's box as it is this draw (no render, no after): drawn already, and its plugin says the user
 *  is editing in it. */
const actHeld = (a, drawn, log) => !!drawn && actAsk(a, 'editing', false, log) === true
/** The badge's markup on the activity bar ('' for none): a count as Threads' found count, '!' as Settings' mark. */
function actBadgeHtml(a, log) {
  const t = a.plugin ? actBadgeText(actAsk(a, 'badge', '', log)) : ''
  if (!t) return ''
  return t === '!' ? '<span class="act-badge" aria-label="Needs your attention">!</span>' : `<span class="act-badge new" aria-label="${esc(t)} new">${esc(t)}</span>`
}
// end of the plugin activity block
