// ── Echo's calendar for a date box: an <input type="date"> opens Echo's menu under it — the days that are picked most
// (today, tomorrow, Friday, next Monday, in a week), the month to pick any day, and "No date" — in place of the
// system's. The box keeps the focus, and a date can still be typed into it; a pick writes it into the box and fires
// `input` and `change`, as the system's calendar does. A box marked `data-native` keeps the system's.

// ── date picker
const DP_DAY = 86400000
const dpIso = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const dpDate = (iso) => { const [y, m, d] = String(iso).split('-').map(Number); return new Date(y, m - 1, d, 12) }
/** The month of `y`, `m` (0-based) as weeks from Monday: [{iso, day, inMonth}] × 7, as many weeks as it spans. */
function dpMonth(y, m) {
  const first = new Date(y, m, 1, 12)
  const start = new Date(first.getTime() - ((first.getDay() + 6) % 7) * DP_DAY)
  const weeks = []
  for (let d = new Date(start); weeks.length < 6; ) {
    const week = []
    for (let i = 0; i < 7; i++, d = new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1, 12)) week.push({ iso: dpIso(d), day: d.getDate(), inMonth: d.getMonth() === m })
    weeks.push(week)
    if (d.getMonth() !== m) break
  }
  return weeks
}
/** The quick picks from `today` (an iso day): Today, Tomorrow, the coming Friday (the next one on a Friday or later
 *  in the week), next Monday, in a week — each {label, iso}; a day already named once isn't named again. */
function dpQuick(today) {
  const t = dpDate(today), plus = (n) => dpIso(new Date(t.getFullYear(), t.getMonth(), t.getDate() + n, 12))
  const dow = (t.getDay() + 6) % 7 // Monday 0
  const picks = [['Today', plus(0)], ['Tomorrow', plus(1)], [dow < 4 ? 'This Friday' : 'Next Friday', plus(dow < 4 ? 4 - dow : 11 - dow)], ['Next Monday', plus(7 - dow)], ['In a week', plus(7)]]
  return picks.filter(([, iso], i) => picks.findIndex(([, x]) => x === iso) === i).map(([label, iso]) => ({ label, iso }))
}
// end of the date picker block

let DP = null // the open calendar: {input, el, y, m}
const dpBox = (t) => t && t.tagName === 'INPUT' && t.type === 'date' && !t.disabled && !t.readOnly && !t.hasAttribute('data-native')
const DP_WD = ['Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa', 'Su']
function dpShort(iso) { return dpDate(iso).toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }) }
function dpDraw() {
  const { input, el, y, m } = DP
  const today = dpIso(new Date()), cur = input.value
  const quick = dpQuick(today).map((q) => `<div class="menu-item dp-q${q.iso === cur ? ' on' : ''}" role="option" data-dp="${q.iso}" aria-selected="${q.iso === cur}"><span>${q.label}</span><span class="dp-qd">${esc(dpShort(q.iso))}</span></div>`).join('')
  const title = new Date(y, m, 1, 12).toLocaleDateString([], { month: 'long', year: 'numeric' })
  const grid = dpMonth(y, m).map((w) => `<div class="dp-wk" role="row">${w.map((d) => `<button type="button" class="dp-d${d.inMonth ? '' : ' out'}${d.iso === today ? ' today' : ''}${d.iso === cur ? ' sel' : ''}" role="gridcell" data-dp="${d.iso}" tabindex="-1" aria-selected="${d.iso === cur}" aria-label="${esc(dpDate(d.iso).toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }))}">${d.day}</button>`).join('')}</div>`).join('')
  el.innerHTML = `<div class="dp-quick" role="listbox" aria-label="Quick dates">${quick}</div>
    <div class="dp-cal"><div class="dp-head"><button type="button" class="icon-btn dp-nav" data-dp-move="-1" tabindex="-1" aria-label="Previous month">‹</button><b class="dp-title">${esc(title)}</b><button type="button" class="icon-btn dp-nav" data-dp-move="1" tabindex="-1" aria-label="Next month">›</button></div>
    <div class="dp-wk dp-wd" aria-hidden="true">${DP_WD.map((x) => `<span>${x}</span>`).join('')}</div><div role="grid" aria-label="${esc(title)}">${grid}</div></div>
    ${cur ? '<div class="menu-sep" role="separator"></div><div class="menu-item dp-none" role="option" data-dp="">No date</div>' : ''}`
  const a = input.getBoundingClientRect()
  const below = innerHeight - a.bottom - 12, up = el.offsetHeight > below && a.top > below
  Object.assign(el.style, { left: `${Math.round(Math.max(8, Math.min(a.left, innerWidth - el.offsetWidth - 8)))}px`, top: `${Math.round(up ? Math.max(8, a.top - 4 - el.offsetHeight) : a.bottom + 4)}px` })
}
function dpOpen(input) {
  dpClose()
  const el = document.createElement('div')
  el.className = 'menu dp-menu'
  el.setAttribute('role', 'dialog')
  el.setAttribute('aria-label', input.getAttribute('aria-label') ? `${input.getAttribute('aria-label')}: pick a day` : 'Pick a day')
  document.body.appendChild(el)
  const at = input.value ? dpDate(input.value) : new Date()
  DP = { input, el, y: at.getFullYear(), m: at.getMonth() }
  input.setAttribute('aria-expanded', 'true')
  dpDraw()
  DP.watch = new MutationObserver(() => { if (DP && !DP.input.isConnected) dpClose() })
  DP.watch.observe(document.body, { childList: true, subtree: true })
}
function dpClose() {
  if (!DP) return
  const { el, input, watch } = DP
  DP = null
  watch.disconnect()
  el.remove()
  input.removeAttribute('aria-expanded')
}
function dpPick(iso) {
  if (!DP) return
  const { input } = DP
  dpClose()
  if (input.value === iso) return
  input.value = iso
  delete input.dataset.typed // picked, not typed: saved at once
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
}
// A press on a date box opens the calendar (the box takes the focus; typing a date still works); a press in the
// calendar keeps the focus in the box and reaches nothing else.
addEventListener('mousedown', (e) => {
  if (DP && DP.el.contains(e.target)) { e.preventDefault(); e.stopPropagation(); return }
  if (dpBox(e.target) && e.button === 0) {
    e.preventDefault()
    if (DP && DP.input === e.target) return // open already (a box opened for an edit opens with it)
    e.target.focus({ preventScroll: true })
    return dpOpen(e.target)
  }
  if (DP) dpClose()
}, true)
for (const t of ['click', 'mouseup', 'pointerdown', 'pointerup']) {
  addEventListener(t, (e) => {
    if (!DP || !DP.el.contains(e.target)) return
    e.stopPropagation()
    if (t !== 'click') return
    e.preventDefault()
    const mv = e.target.closest('[data-dp-move]')
    if (mv) { const d = new Date(DP.y, DP.m + Number(mv.dataset.dpMove), 1, 12); DP.y = d.getFullYear(); DP.m = d.getMonth(); return dpDraw() }
    const p = e.target.closest('[data-dp]')
    if (p) dpPick(p.dataset.dp)
  }, true)
}
// Focus into a date box by the keyboard opens it too; Alt+↓ or F4 opens it; Escape closes it first; leaving the box
// closes it. Typing goes to the box as before.
addEventListener('focusin', (e) => { if (dpBox(e.target) && (!DP || DP.input !== e.target) && e.target.matches(':focus-visible')) dpOpen(e.target) }, true)
addEventListener('focusout', (e) => { if (DP && e.target === DP.input) dpClose() }, true)
addEventListener('keydown', (e) => {
  if (!DP && dpBox(e.target) && (e.key === 'F4' || (e.altKey && e.key === 'ArrowDown'))) { e.preventDefault(); return dpOpen(e.target) }
  if (DP && e.target === DP.input && e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); dpClose() }
}, true)
addEventListener('scroll', (e) => { if (DP && !DP.el.contains(e.target)) dpDraw() }, true)
addEventListener('resize', () => { if (DP) dpDraw() })
