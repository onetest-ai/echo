// ── Echo's menu for the page's selects: a <select> stays the page's state and its `change` handlers; a click or key on
// it opens this menu in place of the system's list, and a pick sets its value and fires `change`. A select marked
// `data-native`, a multiple one or one with a size keeps the system's. An option's `data-sub` is a line under its name.
// As the system's list, the menu never takes the focus: it stays on the select (an area that closes when the focus or a
// click leaves it stays open, the refresh holds while a select has the focus), and the menu's clicks reach nothing else.

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

let SM = null // the open menu: {sel, rows, at, el, typed, typedAt}
const smUses = (sel) => sel && sel.tagName === 'SELECT' && !sel.multiple && !(sel.size > 1) && !sel.disabled && !sel.hasAttribute('data-native')
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
/** What the menu lies on: the select, or the label it lies invisible over (a chip with its own caret). */
const smAnchor = (sel) => (getComputedStyle(sel).opacity === '0' && sel.parentElement ? sel.parentElement : sel)
/** The select's name: its aria-label, else its label's own words (not its options'). */
function smName(sel) {
  if (sel.getAttribute('aria-label')) return sel.getAttribute('aria-label')
  const l = sel.closest('label')
  return l ? [...l.childNodes].filter((n) => n.nodeType === 3).map((n) => n.textContent).join(' ').trim() : ''
}
const SM_CHECK = '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M2.5 6.5l2.5 2.5 4.5-5.5" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/></svg>'
function smOpen(sel) {
  smClose()
  const rows = smRead(sel)
  if (!rows.some(selectable)) return
  const el = document.createElement('div')
  el.className = 'menu sm-menu'
  el.id = 'sm-menu'
  el.setAttribute('role', 'listbox')
  const name = smName(sel)
  if (name) el.setAttribute('aria-label', name)
  el.innerHTML = rows.map((r, i) => (r.kind === 'group'
    ? `<div class="menu-label" role="presentation">${esc(r.label)}</div>`
    : `<div class="menu-item sm-item" role="option" id="sm-opt-${i}" data-sm="${i}" aria-selected="${r.selected}"${r.disabled ? ' aria-disabled="true"' : ''}><span class="sm-check">${r.selected ? SM_CHECK : ''}</span><span class="sm-text"><span class="sm-label">${esc(r.label)}</span>${r.sub ? `<span class="sm-sub">${esc(r.sub)}</span>` : ''}</span></div>`)).join('')
  document.body.appendChild(el)
  const at = rows.findIndex((r) => r.selected && selectable(r))
  SM = { sel, rows, at: at >= 0 ? at : selectEnd(rows, 1), el, typed: '', typedAt: 0, h: el.offsetHeight }
  SM.at0 = SM.at // the row that lies on the field, wherever the arrows go
  if (document.activeElement !== sel) sel.focus({ preventScroll: true })
  sel.setAttribute('aria-expanded', 'true')
  sel.setAttribute('aria-controls', 'sm-menu')
  smPlace()
  smMark()
  // The page drawn anew without its select (its area changed under it): closed, not left over nothing.
  SM.watch = new MutationObserver(() => { if (SM && !SM.sel.isConnected) smClose() })
  SM.watch.observe(document.body, { childList: true, subtree: true })
}
/** On its select where it is now; closed when the select is gone or left the view. */
function smPlace() {
  const sel = SM.sel
  const a = sel.isConnected && smAnchor(sel).getBoundingClientRect()
  if (!a || !a.width || a.bottom < 0 || a.top > innerHeight) return smClose()
  // The chosen row's name (not the line under it) right on the field's text.
  const row = SM.el.querySelector(`[data-sm="${SM.at0}"]`) || SM.el.querySelector('[data-sm]')
  const box = SM.el.getBoundingClientRect(), name = row.querySelector('.sm-label').getBoundingClientRect()
  const pad = parseFloat(getComputedStyle(smAnchor(sel)).paddingLeft) || 0
  const p = selectPlace(a, { width: SM.el.offsetWidth, height: SM.h, rowTop: name.top - box.top + SM.el.scrollTop, rowHeight: name.height, inset: name.left - box.left - pad }, { width: innerWidth, height: innerHeight })
  Object.assign(SM.el.style, { left: `${p.left}px`, top: `${p.top}px`, maxHeight: `${p.maxHeight}px`, minWidth: `${p.minWidth}px` })
  if (!SM.placed) { SM.el.scrollTop = p.scroll; SM.placed = true }
}
function smMark() {
  if (!SM) return
  for (const b of SM.el.querySelectorAll('[data-sm]')) b.classList.toggle('on', +b.dataset.sm === SM.at)
  SM.sel.setAttribute('aria-activedescendant', `sm-opt-${SM.at}`)
  const on = SM.el.querySelector('.sm-item.on')
  if (on && SM.placed) on.scrollIntoView({ block: 'nearest' })
}
function smClose() {
  if (!SM) return
  const { el, sel, watch } = SM
  SM = null
  if (watch) watch.disconnect()
  el.remove()
  for (const a of ['aria-expanded', 'aria-controls', 'aria-activedescendant']) sel.removeAttribute(a)
}
function smPick(i) {
  if (!SM || !selectable(SM.rows[i])) return
  const { sel } = SM, { value } = SM.rows[i]
  smClose()
  if (!sel.isConnected || sel.value === value) return
  sel.value = value
  sel.dispatchEvent(new Event('input', { bubbles: true }))
  sel.dispatchEvent(new Event('change', { bubbles: true }))
  // The page drawn anew for the change: the focus back on its select, when only one on the page is it (by its
  // attributes) and nothing else took the focus — for the keyboard; never a guess between several alike.
  const sign = [...sel.attributes].filter((a) => !['class', 'style', 'aria-expanded', 'aria-controls', 'aria-activedescendant'].includes(a.name)).map((a) => `[${a.name}="${CSS.escape(a.value)}"]`).join('')
  const back = () => {
    const f = document.activeElement
    if (f && f !== document.body) return
    const all = sign ? document.querySelectorAll(`select${sign}`) : []
    if (all.length === 1) all[0].focus({ preventScroll: true })
  }
  for (const ms of [60, 300, 800]) setTimeout(back, ms) // drawn now, and again when the app has answered
}
// On the window, before the page's own handlers: a press on a select opens the menu (the system's list never shows);
// a press or click in the menu reaches nothing else (an area's "clicked outside me" never sees it); one elsewhere closes it.
addEventListener('mousedown', (e) => {
  if (SM && SM.el.contains(e.target)) { e.preventDefault(); e.stopPropagation(); swallow = 0; SM.pressed = 0; return }
  const sel = e.target.closest && e.target.closest('select')
  if (e.button === 0 && smUses(sel)) {
    e.preventDefault()
    const again = SM && SM.sel === sel
    smClose()
    if (!again) { smOpen(sel); if (SM) SM.pressed = Date.now() }
    // The press's own release and click land on the menu (it opens on the field) or the select: they open, nothing more.
    swallow = Date.now()
    return
  }
  smClose()
}, true)
let swallow = 0 // when a press opened or closed the menu: its click is its own
for (const t of ['click', 'mouseup', 'pointerdown', 'pointerup']) {
  addEventListener(t, (e) => {
    const own = (t === 'click' || t === 'mouseup' || t === 'pointerup') && swallow && Date.now() - swallow < 1000
    if (t === 'click') swallow = 0
    if (!(SM && SM.el.contains(e.target)) && !own) return
    e.stopPropagation()
    if (t === 'click') e.preventDefault()
    const b = SM && SM.el.contains(e.target) && e.target.closest('[data-sm]')
    if (!b) return
    // A click on a row picks it; so does a press held on the select and let go on a row (as the Mac's own).
    if (t === 'click' && !own) smPick(+b.dataset.sm)
    else if (t === 'mouseup' && SM.pressed && Date.now() - SM.pressed > 300) smPick(+b.dataset.sm)
  }, true)
}
addEventListener('mousemove', (e) => {
  const b = SM && e.target.closest && e.target.closest('.sm-menu [data-sm]')
  if (b && b.getAttribute('aria-disabled') !== 'true' && +b.dataset.sm !== SM.at) { SM.at = +b.dataset.sm; for (const x of SM.el.querySelectorAll('[data-sm]')) x.classList.toggle('on', x === b); SM.sel.setAttribute('aria-activedescendant', b.id) }
}, true)
// Keys go to the select (it keeps the focus): those that open the system's list open the menu; while it is open,
// the menu's keys move and pick in it and nothing else hears them.
addEventListener('keydown', (e) => {
  const sel = e.target
  const take = () => { e.preventDefault(); e.stopPropagation() }
  if (!SM) {
    if (smUses(sel) && !e.metaKey && !e.ctrlKey && (e.key === ' ' || e.key === 'Enter' || e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'F4')) { take(); smOpen(sel) }
    return
  }
  if (sel !== SM.sel) return smClose()
  const k = e.key
  if (k === 'Escape') { take(); return smClose() }
  if (k === 'Tab') return smClose()
  if (e.metaKey || e.ctrlKey) return
  if (k === 'ArrowDown' || k === 'ArrowUp') { take(); const i = selectStep(SM.rows, SM.at, k === 'ArrowDown' ? 1 : -1); if (i >= 0) { SM.at = i; smMark() } return }
  if (k === 'Home' || k === 'End' || k === 'PageUp' || k === 'PageDown') { take(); const i = selectEnd(SM.rows, k === 'Home' || k === 'PageUp' ? 1 : -1); if (i >= 0) { SM.at = i; smMark() } return }
  const now = Date.now()
  if (now - SM.typedAt > 700) SM.typed = ''
  if (k === 'Enter' || k === 'F4' || (k === ' ' && !SM.typed)) { take(); return smPick(SM.at) }
  if (k.length === 1 && !e.altKey) {
    take()
    SM.typed += k
    SM.typedAt = now
    const i = selectFind(SM.rows, SM.typed.length > 1 ? SM.at - 1 : SM.at, SM.typed)
    if (i >= 0) { SM.at = i; smMark() }
    return
  }
  take() // any other key while it is open changes nothing under it
}, true)
// The focus leaving its select (a click elsewhere, ⌘K, another window) closes it, as the system's list.
addEventListener('focusout', (e) => { if (SM && e.target === SM.sel) setTimeout(() => { if (SM && document.activeElement !== SM.sel) smClose() }, 0) }, true)
// The page scrolling under it or the window resizing moves its select: it follows (its own list scrolls inside).
addEventListener('scroll', (e) => { if (SM && !SM.el.contains(e.target)) smPlace() }, true)
addEventListener('resize', () => { if (SM) smPlace() })
addEventListener('blur', () => smClose())
