// ── Echo's menu for the page's selects: a <select> stays the page's state and its `change` handlers; a click or key on
// it opens this menu in place of the system's list, and a pick sets its value and fires `change`. A select marked
// `data-native`, a multiple one or one with a size keeps the system's. An option's `data-sub` is a line under its name.

// ── select menu
/** A select's options (plain data: {group} or {value, label, sub, selected, disabled}) as the menu's rows. */
function selectItems(opts) {
  return opts.map((o) => (o.group != null ? { kind: 'group', label: String(o.group) } : { kind: 'item', value: String(o.value), label: String(o.label || ''), sub: String(o.sub || ''), selected: !!o.selected, disabled: !!o.disabled }))
}
const selectable = (r) => r && r.kind === 'item' && !r.disabled
/** The row to move to from `at` by `step` (+1 / −1), past groups and disabled rows, wrapping; -1 when none can be. */
function selectStep(rows, at, step) {
  const n = rows.length
  for (let k = 1; k <= n; k++) {
    const i = (((at + step * k) % n) + n) % n
    if (selectable(rows[i])) return i
  }
  return -1
}
/** The first (`step` 1) or last (−1) row that can be picked. */
const selectEnd = (rows, step) => selectStep(rows, step > 0 ? -1 : rows.length, step)
/** Type-ahead: the first row after `at` (wrapping, `at` itself last) whose name starts with `typed`, any case; -1 if none. */
function selectFind(rows, at, typed) {
  const t = String(typed).toLowerCase(), n = rows.length
  if (!t) return -1
  for (let k = 1; k <= n; k++) {
    const i = (at + k + n) % n
    if (selectable(rows[i]) && rows[i].label.toLowerCase().startsWith(t)) return i
  }
  return -1
}
/** Where the menu goes ({left, top, maxHeight, minWidth, scroll}), as the system's own does on a Mac: on its field,
 *  the chosen row (`menu.rowTop`, `rowHeight`) right over it and its text on the field's (`menu.inset`: how much further
 *  in the menu's text starts than the field's); never past the view's edges (8 px), and when it must be moved to stay
 *  inside, its list scrolled (`scroll`) so the chosen row still lies on the field where it can; at least the field's width.
 *  `a` the field ({left, top, height, width}), `view` ({width, height}). */
function selectPlace(a, menu, view) {
  const edge = 8
  const want = a.top + a.height / 2 - (menu.rowTop + menu.rowHeight / 2)
  // Its top past the view's: the list shorter by as much, the rows above the chosen one scrolled away (as the Mac's).
  // A list taller than the view: from where it opens down to the view's bottom, scrolling (a few rows at least).
  const cut = Math.max(0, edge - want), avail = view.height - 2 * edge
  const maxHeight = menu.height - cut <= avail ? Math.max(80, menu.height - cut) : Math.max(120, Math.min(avail, view.height - edge - Math.max(want, edge)))
  const height = Math.min(menu.height, maxHeight)
  const width = Math.max(menu.width, a.width)
  const top = Math.max(edge, Math.min(want, view.height - edge - height))
  const scroll = Math.max(0, Math.min(top - want, menu.height - height))
  const left = Math.max(edge, Math.min(a.left - (menu.inset || 0), view.width - width - edge))
  return { left: Math.round(left), top: Math.round(top), maxHeight, minWidth: Math.round(a.width + (menu.inset || 0)), scroll: Math.round(scroll) }
}
// end of the select menu block

let SM = null // the open menu: {sel, sign, rows, at, el, typed, typedAt}
const smUses = (sel) => sel && sel.tagName === 'SELECT' && !sel.multiple && !(sel.size > 1) && !sel.disabled && !sel.hasAttribute('data-native')
/** How to find the select again when the page was drawn anew under the open menu: its tag's attributes. */
const smSign = (sel) => [...sel.attributes].filter((a) => !['class', 'style', 'aria-expanded'].includes(a.name)).map((a) => `[${a.name}="${CSS.escape(a.value)}"]`).join('')
function smRead(sel) {
  const opts = []
  for (const c of sel.children) {
    if (c.tagName === 'OPTGROUP') {
      opts.push({ group: c.label })
      for (const o of c.children) opts.push({ value: o.value, label: o.textContent, sub: o.dataset.sub, selected: o.selected, disabled: o.disabled || c.disabled })
    } else if (c.tagName === 'OPTION') opts.push({ value: c.value, label: c.textContent, sub: c.dataset.sub, selected: c.selected, disabled: c.disabled })
  }
  return selectItems(opts)
}
/** What the menu hangs under: the select, or the label it lies invisible over (a chip with its own caret). */
const smAnchor = (sel) => (getComputedStyle(sel).opacity === '0' && sel.parentElement ? sel.parentElement : sel)
const SM_CHECK = '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M2.5 6.5l2.5 2.5 4.5-5.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'
function smOpen(sel) {
  smClose()
  const rows = smRead(sel)
  if (!rows.some(selectable)) return
  const el = document.createElement('div')
  el.className = 'menu sm-menu'
  el.setAttribute('role', 'listbox')
  el.tabIndex = -1
  const name = sel.getAttribute('aria-label') || (sel.closest('label') && sel.closest('label').textContent.trim()) || ''
  if (name) el.setAttribute('aria-label', name)
  el.innerHTML = rows.map((r, i) => (r.kind === 'group'
    ? `<div class="menu-label" role="presentation">${esc(r.label)}</div>`
    : `<button type="button" class="menu-item sm-item${r.sub ? ' two' : ''}" role="option" data-sm="${i}" aria-selected="${r.selected}"${r.disabled ? ' disabled' : ''}><span class="sm-check">${r.selected ? SM_CHECK : ''}</span><span class="sm-text"><span class="sm-label">${esc(r.label)}</span>${r.sub ? `<span class="sm-sub">${esc(r.sub)}</span>` : ''}</span></button>`)).join('')
  document.body.appendChild(el)
  const at = rows.findIndex((r) => r.selected && selectable(r))
  SM = { sel, sign: smSign(sel), rows, at: at >= 0 ? at : selectEnd(rows, 1), el, typed: '', typedAt: 0, h: el.offsetHeight }
  SM.at0 = SM.at // the row that lies on the field, wherever the arrows go
  smPlace()
  sel.setAttribute('aria-expanded', 'true')
  smMark()
  el.focus({ preventScroll: true })
}
/** On its select where it is now; closed when the select left the view (scrolled away, or drawn anew elsewhere). */
function smPlace() {
  let sel = SM.sel
  if (!sel.isConnected) sel = (SM.sign && document.querySelector(`select${SM.sign}`)) || sel
  const a = sel.isConnected && smAnchor(sel).getBoundingClientRect()
  if (!a || !a.width || a.bottom < 0 || a.top > innerHeight) return smClose()
  SM.sel = sel
  // Its text on the field's: an invisible select over a chip has the chip's own padding.
  const row = SM.el.querySelector(`[data-sm="${SM.at0}"]`) || SM.el.querySelector('[data-sm]')
  // The chosen row's name (not the line under it) on the field's text.
  const box = SM.el.getBoundingClientRect(), name = row.querySelector('.sm-label').getBoundingClientRect()
  const pad = parseFloat(getComputedStyle(smAnchor(sel)).paddingLeft) || 0
  const p = selectPlace(a, { width: SM.el.offsetWidth, height: SM.h, rowTop: name.top - box.top + SM.el.scrollTop, rowHeight: name.height, inset: name.left - box.left - pad }, { width: innerWidth, height: innerHeight })
  Object.assign(SM.el.style, { left: `${p.left}px`, top: `${p.top}px`, maxHeight: `${p.maxHeight}px`, minWidth: `${p.minWidth}px` })
  if (!SM.placed) { SM.el.scrollTop = p.scroll; SM.placed = true }
}
function smMark() {
  if (!SM) return
  for (const b of SM.el.querySelectorAll('[data-sm]')) b.classList.toggle('on', +b.dataset.sm === SM.at)
  const on = SM.el.querySelector('.sm-item.on')
  if (on) on.scrollIntoView({ block: 'nearest' })
}
function smClose(refocus) {
  if (!SM) return
  const { el, sel } = SM
  SM = null
  el.remove()
  if (sel.isConnected) {
    sel.removeAttribute('aria-expanded')
    if (refocus) sel.focus({ preventScroll: true })
  }
}
function smPick(i) {
  if (!SM || !selectable(SM.rows[i])) return
  const { value } = SM.rows[i]
  // Drawn anew while open: the same select by its attributes.
  let sel = SM.sel
  if (!sel.isConnected) sel = (SM.sign && document.querySelector(`select${SM.sign}`)) || sel
  SM.sel = sel
  smClose(true)
  if (!sel.isConnected || sel.value === value) return
  sel.value = value
  sel.dispatchEvent(new Event('input', { bubbles: true }))
  sel.dispatchEvent(new Event('change', { bubbles: true }))
  // The page drawn anew for the change: the focus back on the select, for the keyboard.
  const sign = smSign(sel)
  setTimeout(() => { const f = document.activeElement; if (!f || f === document.body) { const s = document.querySelector(`select${sign}`); if (s) s.focus({ preventScroll: true }) } }, 60)
}
// A press on a select opens the menu (the system's list never shows); a press elsewhere closes it.
document.addEventListener('mousedown', (e) => {
  if (SM && SM.el.contains(e.target)) { e.preventDefault(); return }
  const sel = e.target.closest && e.target.closest('select')
  if (e.button === 0 && smUses(sel)) {
    e.preventDefault()
    const again = SM && SM.sel === sel
    smClose()
    if (!again) { sel.focus({ preventScroll: true }); smOpen(sel) }
    return
  }
  smClose()
}, true)
// A row's click picks it; nothing under the menu sees the click (a menu it was opened from stays open).
document.addEventListener('click', (e) => {
  if (!SM || !SM.el.contains(e.target)) return
  e.preventDefault(); e.stopPropagation()
  const b = e.target.closest('[data-sm]')
  if (b) smPick(+b.dataset.sm)
}, true)
document.addEventListener('mousemove', (e) => {
  const b = SM && e.target.closest && e.target.closest('.sm-menu [data-sm]')
  if (b && !b.disabled && +b.dataset.sm !== SM.at) { SM.at = +b.dataset.sm; smMark() }
}, true)
document.addEventListener('keydown', (e) => {
  if (!SM) {
    // On a select: the keys that would open the system's list open the menu.
    const sel = e.target
    if (smUses(sel) && !e.metaKey && !e.ctrlKey && (e.key === ' ' || e.key === 'Enter' || e.key === 'ArrowDown' || e.key === 'ArrowUp')) { e.preventDefault(); e.stopPropagation(); smOpen(sel) }
    return
  }
  const k = e.key
  const take = () => { e.preventDefault(); e.stopPropagation() }
  if (k === 'Escape') { take(); return smClose(true) }
  if (k === 'Tab') return smClose(true)
  if (k === 'ArrowDown' || k === 'ArrowUp') { take(); const i = selectStep(SM.rows, SM.at, k === 'ArrowDown' ? 1 : -1); if (i >= 0) { SM.at = i; smMark() } return }
  if (k === 'Home' || k === 'End' || k === 'PageUp' || k === 'PageDown') { take(); const i = selectEnd(SM.rows, k === 'Home' || k === 'PageUp' ? 1 : -1); if (i >= 0) { SM.at = i; smMark() } return }
  if (k === 'Enter' || (k === ' ' && !SM.typed)) { take(); return smPick(SM.at) }
  if (k.length === 1 && !e.metaKey && !e.ctrlKey && !e.altKey) {
    take()
    const now = Date.now()
    SM.typed = now - SM.typedAt > 700 ? k : SM.typed + k
    SM.typedAt = now
    const i = selectFind(SM.rows, SM.typed.length > 1 ? SM.at - 1 : SM.at, SM.typed)
    if (i >= 0) { SM.at = i; smMark() }
  }
}, true)
// The page scrolling under it or the window resizing moves its select: it follows (its own list scrolls inside).
document.addEventListener('scroll', (e) => { if (SM && !SM.el.contains(e.target)) smPlace() }, true)
addEventListener('resize', () => { if (SM) smPlace() })
addEventListener('blur', () => smClose())
