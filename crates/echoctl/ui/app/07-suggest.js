// ── Echo's suggestions for a text box: an <input list="…"> shows its <datalist>'s options in Echo's menu, under the box,
// in place of the system's. The box keeps the focus and stays free text: a typed name that isn't there is the value
// as before; a picked one is written into the box and fires `input` and `change`, as typing it and leaving would.
// An option's `data-sub` is a line beside its name; a datalist with `data-people` shows a face for each name, and
// its option with an empty value ("Nobody named") clears the box.

// ── suggest
/** The rows a box shows ({value, label, sub, empty}) for what is typed: every one when nothing is, else those whose
 *  name or line has it (any case), the ones starting with it first; an empty-value row (clears it) always last. */
function suggestRows(opts, typed) {
  const t = String(typed || '').trim().toLowerCase()
  const rows = opts.map((o) => ({ value: String(o.value), label: String(o.label || o.value), sub: String(o.sub || ''), empty: String(o.value) === '' }))
  const clear = rows.filter((r) => r.empty), named = rows.filter((r) => !r.empty)
  if (!t) return [...named, ...clear]
  const has = named.filter((r) => r.label.toLowerCase().includes(t) || r.sub.toLowerCase().includes(t))
  const starts = (r) => (r.label.toLowerCase().startsWith(t) ? 0 : 1)
  return [...has.sort((a, b) => starts(a) - starts(b)), ...clear]
}
// end of the suggest block

let SG = null // the open list: {input, list, rows, at, el, people}
function sgOpts(list) {
  return [...list.querySelectorAll('option')].map((o) => ({ value: o.value, label: o.label || o.textContent || o.value, sub: o.dataset.sub }))
}
function sgDraw() {
  const { input, el, people } = SG
  SG.rows = suggestRows(sgOpts(SG.list), input.value === SG.start ? '' : input.value)
  if (!SG.rows.length) { el.hidden = true; return }
  el.hidden = false
  if (SG.at >= SG.rows.length) SG.at = -1
  const cur = SG.start.trim().toLowerCase()
  el.innerHTML = SG.rows.map((r, i) => `${r.empty && i ? '<div class="menu-sep" role="separator"></div>' : ''}<div class="menu-item sm-item sg-item${i === SG.at ? ' on' : ''}" role="option" id="sg-opt-${i}" data-sg="${i}" aria-selected="${!r.empty && r.label.toLowerCase() === cur}">${people ? (r.empty ? '<span class="bd-av none">–</span>' : avatar(r.label)) : `<span class="sm-check">${!r.empty && r.label.toLowerCase() === cur ? SM_CHECK : ''}</span>`}<span class="sm-text"><span class="sm-label">${esc(r.label)}</span></span>${r.sub ? `<span class="sg-sub">${esc(r.sub)}</span>` : ''}</div>`).join('')
  const a = input.getBoundingClientRect()
  const below = innerHeight - a.bottom - 12, up = el.offsetHeight > below && a.top > below
  Object.assign(el.style, { left: `${Math.round(Math.max(8, Math.min(a.left, innerWidth - el.offsetWidth - 8)))}px`, top: `${Math.round(up ? a.top - 4 - Math.min(el.offsetHeight, a.top - 12) : a.bottom + 4)}px`, minWidth: `${Math.round(a.width)}px`, maxHeight: `${Math.round(Math.max(120, up ? a.top - 12 : below))}px` })
  input.setAttribute('aria-activedescendant', SG.at >= 0 ? `sg-opt-${SG.at}` : '')
}
function sgOpen(input) {
  sgClose()
  const id = input.getAttribute('list') || input.dataset.suggest
  const list = id && document.getElementById(id)
  if (!list) return
  // The system's own list never shows: the box points at its datalist by `data-suggest` from now on.
  input.dataset.suggest = id
  input.removeAttribute('list')
  const el = document.createElement('div')
  el.className = 'menu sm-menu sg-menu'
  el.id = 'sg-menu'
  el.setAttribute('role', 'listbox')
  if (input.getAttribute('aria-label')) el.setAttribute('aria-label', input.getAttribute('aria-label'))
  document.body.appendChild(el)
  SG = { input, list, el, at: -1, start: input.value, people: list.hasAttribute('data-people') }
  input.setAttribute('role', 'combobox')
  input.setAttribute('aria-autocomplete', 'list')
  input.setAttribute('aria-controls', 'sg-menu')
  input.setAttribute('aria-expanded', 'true')
  sgDraw()
  SG.watch = new MutationObserver(() => { if (SG && !SG.input.isConnected) sgClose() })
  SG.watch.observe(document.body, { childList: true, subtree: true })
}
function sgClose() {
  if (!SG) return
  const { el, input, watch } = SG
  SG = null
  watch.disconnect()
  el.remove()
  input.setAttribute('aria-expanded', 'false')
  input.removeAttribute('aria-activedescendant')
  input.removeAttribute('aria-controls')
}
function sgPick(i) {
  if (!SG || !SG.rows[i]) return
  const { input } = SG, r = SG.rows[i]
  sgClose()
  input.value = r.empty ? '' : r.label
  input.dispatchEvent(new Event('input', { bubbles: true }))
  input.dispatchEvent(new Event('change', { bubbles: true }))
  sgPicked = { input, at: Date.now() }
}
// A box typed in and then picked from would fire its own `change` too when it is left: the pick's is the one.
let sgPicked = null
addEventListener('change', (e) => {
  if (e.isTrusted && sgPicked && e.target === sgPicked.input && Date.now() - sgPicked.at < 2000) { e.stopImmediatePropagation(); e.stopPropagation() }
}, true)
const sgBox = (t) => t && t.tagName === 'INPUT' && (t.getAttribute('list') || t.dataset.suggest)
addEventListener('focusin', (e) => { if (sgBox(e.target)) sgOpen(e.target) }, true)
addEventListener('focusout', (e) => { if (SG && e.target === SG.input) sgClose() }, true)
addEventListener('input', (e) => { if (SG && e.target === SG.input) { SG.at = -1; sgDraw() } }, true)
// A press in the list keeps the focus in the box (its area stays open) and reaches nothing else; a click picks.
addEventListener('mousedown', (e) => { if (SG && SG.el.contains(e.target)) { e.preventDefault(); e.stopPropagation() } }, true)
for (const t of ['click', 'mouseup', 'pointerdown', 'pointerup']) {
  addEventListener(t, (e) => {
    if (!SG || !SG.el.contains(e.target)) return
    e.stopPropagation()
    if (t !== 'click') return
    e.preventDefault()
    const b = e.target.closest('[data-sg]')
    if (b) sgPick(+b.dataset.sg)
  }, true)
}
addEventListener('mousemove', (e) => {
  const b = SG && e.target.closest && e.target.closest('.sg-menu [data-sg]')
  if (b && +b.dataset.sg !== SG.at) { SG.at = +b.dataset.sg; SG.kbd = false; for (const x of SG.el.querySelectorAll('[data-sg]')) x.classList.toggle('on', x === b); SG.input.setAttribute('aria-activedescendant', b.id) }
}, true)
// ↑ ↓ move in the list; Enter picks the row the keys marked (none, or only one under the mouse: the box's own text, as
// before); Escape closes the list
// and is the box's own Escape too.
addEventListener('keydown', (e) => {
  if (!SG || e.target !== SG.input || SG.el.hidden) return
  const take = () => { e.preventDefault(); e.stopPropagation() }
  const n = SG.rows.length
  if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
    take()
    SG.at = e.key === 'ArrowDown' ? (SG.at + 1) % n : (SG.at <= 0 ? n - 1 : SG.at - 1)
    SG.kbd = true
    for (const x of SG.el.querySelectorAll('[data-sg]')) x.classList.toggle('on', +x.dataset.sg === SG.at)
    const on = SG.el.querySelector('.sg-item.on'); if (on) on.scrollIntoView({ block: 'nearest' })
    SG.input.setAttribute('aria-activedescendant', `sg-opt-${SG.at}`)
  } else if (e.key === 'Enter' && SG.at >= 0 && SG.kbd) { take(); sgPick(SG.at) } else if (e.key === 'Escape') sgClose() // and the box's own Escape
}, true)
addEventListener('scroll', (e) => { if (SG && !SG.el.contains(e.target)) sgDraw() }, true)
addEventListener('resize', () => { if (SG) sgDraw() })
