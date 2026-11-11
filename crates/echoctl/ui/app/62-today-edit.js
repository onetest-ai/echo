// ── Today's edit mode and Add a widget (#315; Workbench `WidgetsGallery`, its working prototype): Edit frames each
// widget — grip, name, S M L, × — drag it to move (a widget's top or bottom quarter: a new row above / below; its left or
// right half: before / after it; the end: last); Add a widget on the right: search, parts, a row a widget, the picked
// one previewed at its sizes, Add or drag it in. Every change is `today_layout` (window/today.rs) → today_layout::place,
// as `echoctl today` does.

const TW_SIZES = { S: 1, M: 2, L: 3 }

/** The head's buttons: Edit; in edit mode Reset to the default and Done. */
function todayEditHead() {
  return S.tdEdit
    ? '<span class="td-edit-acts"><button class="btn" data-tw-panel="1">Add a widget</button><button class="btn ghost" data-tw-reset="1">Reset to the default</button><button class="btn primary" data-tw-done="1">Done</button></span>'
    : `<button class="icon-btn td-edit-btn" data-tw-edit="1" aria-label="Edit Today" title="Edit Today — move, size, add widgets">${ICON('<path d="M4 20h4L19 9l-4-4L4 16z"/><path d="M14 6l4 4"/>')}</button>`
}

/** A widget framed for editing: its name, the sizes it takes, ×; its content shown but inert; empty: what it is. */
function todayEditCell(id, html, size) {
  const w = (S.tdCatalog || []).find((x) => x.id === id) || { id, label: id, sizes: ['S', 'M', 'L'] }
  const sizes = (w.sizes || []).map((z) => `<button type="button" class="${z === size ? 'on' : ''}" data-tw-size="${esc(z)}" data-tw-id="${esc(id)}" aria-pressed="${z === size}" aria-label="${esc(w.label)}: size ${esc(z)}">${esc(z)}</button>`).join('')
  const rows = twRows(), r = rows.findIndex((row) => row.includes(id)), i = r < 0 ? -1 : rows[r].indexOf(id)
  const can = { up: r > 0, down: r >= 0 && r < rows.length - 1, left: i > 0, right: i >= 0 && i < (rows[r] || []).length - 1 }
  const MV = { left: ['←', 'before the one beside it'], up: ['↑', 'to a row above'], down: ['↓', 'to a row below'], right: ['→', 'after the one beside it'] }
  const moves = Object.entries(MV).filter(([k]) => can[k]).map(([k, [a, say]]) => `<button type="button" data-tw-move="${k}" data-tw-id="${esc(id)}" aria-label="Move ${esc(w.label)} ${say}" title="Move ${say}">${a}</button>`).join('')
  // Its settings (⚙): a select a choice, thread or client, a box a yes/no; changed, Today's layout says it.
  const item = ((S.today && S.today.layout) || []).find((i) => i.widget === id) || {}
  const optsOf = (p) => (p.type === 'choice' ? p.options.map((o) => [o, o])
    : p.type === 'thread' ? ((S.pj.data && S.pj.data.projects) || []).filter((t) => t.group !== 'closed').map((t) => [t.id, t.name])
    : p.type === 'client' ? ((S.pj.data && S.pj.data.clients) || []).map((c) => [c.id, c.name]) : [])
  // What the widget uses: the value set if it still takes it, else its default (as `params_of` does).
  const now = (p) => {
    const v = item.params && item.params[p.key]
    if (v !== undefined && (p.type === 'bool' ? typeof v === 'boolean' : optsOf(p).some(([o]) => o === v))) return v
    return p.default
  }
  const pick = (p) => {
    const opts = optsOf(p)
    // An id of plain characters: the render gives a changed setting its focus back (#395).
    const fid = `tw-p-${`${id}-${p.key}`.replace(/[^a-z0-9-]/gi, '-')}`
    if (p.type === 'bool') return `<label class="tw-param"><input id="${fid}" type="checkbox" data-tw-param="${esc(p.key)}" data-tw-id="${esc(id)}" ${now(p) ? 'checked' : ''}> ${esc(p.label)}</label>`
    return `<label class="tw-param">${esc(p.label)} <select id="${fid}" data-tw-param="${esc(p.key)}" data-tw-id="${esc(id)}">${now(p) === undefined ? '<option value="">—</option>' : ''}${opts.map(([v, l]) => `<option value="${esc(v)}" ${v === now(p) ? 'selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  }
  const params = (w.params || []).length
  const gear = params ? `<button type="button" id="tw-gear-${esc(id.replace(/[^a-z0-9-]/gi, '-'))}" class="icon-btn tw-gear${S.tdParams === id ? ' on' : ''}" data-tw-gear="${esc(id)}" aria-expanded="${S.tdParams === id}" aria-label="${esc(w.label)}: settings" title="Settings">${ICON('<circle cx="12" cy="12" r="3"/><path d="M12 3v2.5M12 18.5V21M3 12h2.5M18.5 12H21M5.6 5.6l1.8 1.8M16.6 16.6l1.8 1.8M5.6 18.4l1.8-1.8M16.6 7.4l1.8-1.8"/>')}</button>` : ''
  const form = params && S.tdParams === id ? `<div class="tw-params" role="group" aria-label="Settings of ${esc(w.label)}">${w.params.map(pick).join('')}</div>` : ''
  const body = html || `<div class="tw-empty set-sub">${esc(w.about || '')}${w.temporal ? ' — shows only while it has something' : ''}</div>`
  return `<section class="tw${w.temporal ? ' tmp' : ''}" draggable="true" data-tw="${esc(id)}" aria-label="${esc(w.label)}"><div class="tw-h"><span class="tw-grip" aria-hidden="true">⠿</span><b>${esc(w.label)}</b>${moves ? `<span class="tw-mv" role="group" aria-label="Move">${moves}</span>` : ''}<span class="tw-seg" role="group" aria-label="Size">${sizes}</span>${gear}<button type="button" class="icon-btn tw-x" data-tw-remove="${esc(id)}" aria-label="Take ${esc(w.label)} off Today" title="Take it off Today">${ICON('<path d="M6 6l12 12M18 6L6 18"/>')}</button></div>${form}<div class="tw-in" inert>${body}</div></section>`
}

/** The zone after the last widget: a widget dropped there goes last, on a row of its own. */
function todayEditEnd() {
  return '<div class="tw-end" data-tw-end="1">Drop here for a new last row · on a widget: top / bottom edge a new row, left / right half beside it</div>'
}

// ── drop side: where a widget dropped over another goes (no DOM: `today-edit.test.mjs`) ─────────────────────────────
/** Over a widget's box `r` at (x, y): its top quarter → 'above' (a row of its own), bottom quarter → 'below', else its
 *  left half → 'before', right half → 'after' (in its row). */
function dropSide(r, x, y) {
  const fy = (y - r.top) / r.height
  return fy < 0.25 ? 'above' : fy > 0.75 ? 'below' : x < r.left + r.width / 2 ? 'before' : 'after'
}
// end of the drop side block

/** A change to Today's layout; its reply refreshes Today (window.echo.refresh) once it is written. */
function todayLayoutDo(body) {
  send({ cmd: 'today_layout', ...body })
}

/** The size the panel offers a widget: the one picked if it takes it, else M, else its first. */
function twPickSize(w) {
  const sizes = (w && w.sizes) || []
  return sizes.includes(S.tdPickSize) ? S.tdPickSize : sizes.includes('M') ? 'M' : sizes[0]
}

/** Add a widget, shown again (closed while editing, or under the page at a narrow width: over it, as the right panel
 *  opens there). */
function todayEditPanel() {
  const known = panelViews()
  if (Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay).narrow) S.overlay = true
  Layout.show('add-widget', known)
  layoutChanged()
}

/** The layout's rows as the page draws them: widget ids, a row each (widgets not drawn left out). */
function twRows() {
  return todayRows((S.today && S.today.layout) || [], Object.keys(S.tdParts || {}))
}

/** A move by keyboard (#345): up — a row of its own above the row before; down — below the row after; left / right —
 *  before the one before it / after the one after it in its row. The moved widget's button keeps the focus. */
function todayEditMove(id, dir) {
  const rows = twRows(), r = rows.findIndex((row) => row.includes(id))
  if (r < 0) return
  const i = rows[r].indexOf(id)
  const to = dir === 'up' ? r > 0 && [rows[r - 1][0], 'above']
    : dir === 'down' ? r < rows.length - 1 && [rows[r + 1][rows[r + 1].length - 1], 'below']
    : dir === 'left' ? i > 0 && [rows[r][i - 1], 'before']
    : i < rows[r].length - 1 && [rows[r][i + 1], 'after']
  if (!to) return
  S.tdFocus = { id, sel: `[data-tw-move="${dir}"]`, was: JSON.stringify(S.today.layout), at: Date.now() }
  todayLayoutDo({ op: 'move', widget: id, target: to[0], side: to[1] })
}

/** After a redraw in edit mode: the focus back where the keyboard was (a moved widget's button, a neighbour after ×). */
function twRefocus() {
  const f = S.tdFocus
  if (!f) return
  // A change refused (its notice said why) never redraws: the focus wish lapses rather than land on a later change.
  if (!S.tdEdit || Date.now() - f.at > 5000) { S.tdFocus = null; return }
  // Only once Today is drawn from the changed layout (the change's reply refreshes it).
  if (JSON.stringify((S.today || {}).layout) === f.was) return
  const frame = document.querySelector(`.tw[data-tw="${CSS.escape(f.id)}"]`)
  const el = frame && (frame.querySelector(f.sel) || frame.querySelector('button:not([disabled])'))
  if (el) { el.focus(); S.tdFocus = null }
}

async function todayEditOpen() {
  S.tdEdit = true
  // As the right panel was, for Done to put it back.
  const was = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay).right
  S.tdEditWas = { open: !!(was && was.shown), active: was && was.active, overlay: S.overlay }
  if (!S.tdCatalog) { try { S.tdCatalog = await get('api/widgets') } catch (e) { S.tdCatalog = [] } }
  if (!S.tdPick) S.tdPick = (S.tdCatalog[0] || {}).id
  const known = panelViews()
  // At a narrow width the panel would cover the widgets' controls: it waits for "Add a widget" in the head (#395).
  if (!Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay).narrow) Layout.show('add-widget', known)
  layoutChanged()
}

/** Done: no frames; the right panel as it was before Edit (closed, or back to the view it showed). */
function todayEditDone() {
  S.tdEdit = false
  S.tdFocus = null
  const was = S.tdEditWas || { open: true, overlay: S.overlay }
  const known = panelViews()
  if (was.active && known.some((v) => v.id === was.active)) Layout.show(was.active, known)
  if (!was.open) Layout.close('right')
  S.overlay = was.overlay
  S.tdEditWas = null
  layoutChanged()
}

/** Add a widget (the right panel in edit mode): search, parts, a row a widget, the picked one's preview and Add. */
function addWidgetPanel() {
  const all = S.tdCatalog || []
  const on = new Set(((S.today && S.today.layout) || []).map((i) => i.widget))
  const parts = ['All', ...new Set(all.map((w) => w.part))]
  const part = S.tdPart || 'All', q = (S.tdQ || '').trim().toLowerCase()
  const shown = all.filter((w) => (part === 'All' || w.part === part) && (!q || `${w.label} ${w.about} ${w.part}`.toLowerCase().includes(q)))
  // The picked one is among what the part and search show (else the first of them; none shown: no preview).
  if (shown.length && !shown.some((x) => x.id === S.tdPick)) S.tdPick = shown[0].id
  const rows = shown.map((w) => `<div class="tw-row${S.tdPick === w.id ? ' sel' : ''}" draggable="true" data-tw-drag="${esc(w.id)}" data-tw-pick="${esc(w.id)}" role="listitem" aria-label="${esc(w.label)}"><span><b>${esc(w.label)}</b>${w.temporal ? ' <span class="tw-tag" title="Shows only while it has something">when needed</span>' : ''}<br><span class="set-sub">${esc(w.about)}</span></span>${on.has(w.id) ? '<span class="tw-on">on Today</span>' : `<button class="btn sm" data-tw-add="${esc(w.id)}" aria-label="Add ${esc(w.label)}">Add</button>`}</div>`).join('') || '<p class="set-sub">No widget matches.</p>'
  const w = shown.find((x) => x.id === S.tdPick)
  let pick = ''
  if (w) {
    const size = twPickSize(w)
    const px = { S: 170, M: 280, L: 380 }[size] || 280
    const html = (S.tdParts || {})[w.id]
    const pv = html ? `<div class="tw-pv-in" inert style="width:${px * 2}px">${html}</div>` : `<div class="set-sub tw-pv-none">${esc(w.about)}${w.temporal ? ' — shows only while it has something' : ''}</div>`
    const dots = ['S', 'M', 'L'].map((z) => `<button type="button" class="tw-dot${z === size ? ' on' : ''}" data-tw-pick-size="${z}" ${(w.sizes || []).includes(z) ? '' : 'disabled'} aria-pressed="${z === size}">${z}</button>`).join('')
    const note = `${size} · ${TW_SIZES[size]} column${TW_SIZES[size] === 1 ? '' : 's'}${w.height === 'fill' ? ' · fills its column' : ''}`
    pick = `<div class="tw-pick"><div class="tw-pick-h"><b>${esc(w.label)}</b><span class="mono">${esc(w.part)}</span></div><div class="tw-pv" style="width:${px}px" draggable="true" data-tw-drag="${esc(w.id)}" aria-label="Preview of ${esc(w.label)} at ${esc(size)}">${pv}</div><div class="tw-dots" role="group" aria-label="Size">${dots}</div><span class="set-sub tw-note">${esc(note)}</span>${on.has(w.id) ? `<button class="btn" data-tw-size="${esc(size)}" data-tw-id="${esc(w.id)}">On Today — make it ${esc(size)}</button>` : `<button class="btn primary" data-tw-add="${esc(w.id)}" data-tw-add-size="${esc(size)}">Add at ${esc(size)}</button>`}<span class="set-sub tw-note">Or drag the preview onto Today.</span></div>`
  }
  // Done here too: at a narrow width this panel is over the page's head.
  return `<div class="tw-panel"><div class="tw-top"><div class="tw-top-acts"><button class="btn primary sm" data-tw-done="1">Done</button></div><input id="tw-q" class="tw-q" type="search" placeholder="Search widgets" aria-label="Search widgets" value="${esc(S.tdQ || '')}" data-tw-q="1"><div class="tw-parts" role="group" aria-label="Parts">${parts.map((p) => `<button type="button" class="tw-part${p === part ? ' on' : ''}" data-tw-part="${esc(p)}" aria-pressed="${p === part}">${esc(p)}</button>`).join('')}</div></div><div class="tw-list" role="list" aria-label="Widgets">${rows}</div>${pick}</div>`
}

PANEL_VIEWS.push({
  id: 'add-widget', label: 'Add a widget', area: 'right',
  icon: ICON('<rect x="4" y="4" width="7" height="7" rx="1.5"/><rect x="13" y="4" width="7" height="7" rx="1.5"/><rect x="4" y="13" width="7" height="7" rx="1.5"/><path d="M16.5 13.5v6M13.5 16.5h6"/>'),
  when: () => S.view === 'today' && !!S.tdEdit,
  render: addWidgetPanel,
})

document.addEventListener('click', (e) => {
  if (S.view !== 'today' || !e.target.closest) return
  const t = e.target.closest('[data-tw-gear],[data-tw-edit],[data-tw-panel],[data-tw-move],[data-tw-done],[data-tw-reset],[data-tw-size],[data-tw-remove],[data-tw-add],[data-tw-pick],[data-tw-pick-size],[data-tw-part]')
  if (!t) return
  const d = t.dataset
  if (d.twGear) { S.tdParams = S.tdParams === d.twGear ? null : d.twGear; return render() }
  if (d.twEdit) return todayEditOpen()
  if (d.twPanel) return todayEditPanel()
  if (d.twMove) return todayEditMove(d.twId, d.twMove)
  if (d.twDone) return todayEditDone()
  if (d.twReset) return todayLayoutDo({ op: 'reset' })
  if (d.twSize) return todayLayoutDo({ op: 'size', widget: d.twId, size: d.twSize })
  if (d.twRemove) {
    const all = twRows().flat(), i = all.indexOf(d.twRemove), next = all[i + 1] || all[i - 1]
    if (next) S.tdFocus = { id: next, sel: '[data-tw-remove]', was: JSON.stringify(S.today.layout), at: Date.now() }
    return todayLayoutDo({ op: 'remove', widget: d.twRemove })
  }
  if (d.twAdd) { e.stopPropagation(); return todayLayoutDo({ op: 'add', widget: d.twAdd, size: d.twAddSize || undefined }) }
  if (d.twPickSize) { S.tdPickSize = d.twPickSize; return render() }
  if (d.twPart) { S.tdPart = d.twPart; return render() }
  if (d.twPick) { S.tdPick = d.twPick; return render() }
})
document.addEventListener('input', (e) => {
  if (!e.target.dataset || !e.target.dataset.twQ) return
  // Drawn again, the search keeps the keyboard and its caret (its id: the render's focus restore).
  S.tdQ = e.target.value; S.tdPart = 'All'; render()
})

// Dragging: a widget on Today (move) or one from the panel (add at the picked size).
let twDrag = null
const twClear = () => document.querySelectorAll('[data-drop]').forEach((x) => x.removeAttribute('data-drop'))
document.addEventListener('dragstart', (e) => {
  if (S.view !== 'today' || !S.tdEdit || !e.target.closest) return
  const page = e.target.closest('.tw[data-tw]'), cat = e.target.closest('[data-tw-drag]')
  if (!page && !cat) return
  const w = cat && (S.tdCatalog || []).find((x) => x.id === cat.dataset.twDrag)
  twDrag = page ? { from: 'page', id: page.dataset.tw } : { from: 'catalog', id: cat.dataset.twDrag, size: S.tdPick === cat.dataset.twDrag ? twPickSize(w) : undefined }
  try { e.dataTransfer.setData('text/plain', twDrag.id); e.dataTransfer.effectAllowed = 'move' } catch (x) { /* some webviews refuse */ }
  if (page) page.classList.add('dragging')
})
document.addEventListener('dragover', (e) => {
  if (!twDrag || !e.target.closest) return
  const w = e.target.closest('.tw[data-tw]'), end = e.target.closest('[data-tw-end]')
  if (!w && !end) return
  e.preventDefault()
  const side = w ? dropSide(w.getBoundingClientRect(), e.clientX, e.clientY) : 'last'
  const el = w || end
  if (el.dataset.drop !== side) { twClear(); el.dataset.drop = side }
})
document.addEventListener('drop', (e) => {
  if (!twDrag || !e.target.closest) return
  const w = e.target.closest('.tw[data-tw]'), end = e.target.closest('[data-tw-end]')
  if (!w && !end) return
  e.preventDefault()
  const drag = twDrag
  twDrag = null
  twClear()
  if (w && w.dataset.tw === drag.id) return render()
  const side = w ? dropSide(w.getBoundingClientRect(), e.clientX, e.clientY) : undefined
  todayLayoutDo({ op: drag.from === 'page' ? 'move' : 'add', widget: drag.id, size: drag.size, target: w ? w.dataset.tw : undefined, side })
})
// A drop anywhere ends the drag, even where no widget takes it (a redraw may have removed the dragged one).
document.addEventListener('drop', () => { if (twDrag) { twDrag = null; twClear() } })
document.addEventListener('dragend', () => {
  if (!twDrag) return
  twDrag = null
  twClear()
  document.querySelectorAll('.tw.dragging').forEach((x) => x.classList.remove('dragging'))
})

// A widget's setting changed in its ⚙ form: Today's layout keeps it (the widget then shows what it says).
document.addEventListener('change', (e) => {
  const el = e.target
  if (S.view !== 'today' || !el.dataset || !el.dataset.twParam) return
  const value = el.type === 'checkbox' ? el.checked : el.value
  if (value === '') return
  todayLayoutDo({ op: 'params', widget: el.dataset.twId, key: el.dataset.twParam, value })
})
