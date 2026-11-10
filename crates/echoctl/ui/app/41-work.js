// ── the Work tab: the project's ledger (`ledger.rs`) ────────────────────────────────────────
// ── work-status: what the tree and the board say of an item, and what the filters keep (no DOM;
//    work-status.test.mjs runs it, with `ymd` from the calendar's helpers)
const CLOSED_STATES = ['done', 'cancelled', 'merged', 'deleted']
const STATE_LABEL = { open: 'to do', in_progress: 'in progress', blocked: 'blocked', done: 'done', cancelled: 'cancelled', merged: 'merged', deleted: 'deleted' }
const KIND_LABEL = { session: 'session', dependency: 'dependency', decision: 'decision', risk: 'risk', question: 'question' }
/** What each kind holds in its Markdown (`description`): the work's description, a question's answer,
 *  the decision itself, what is done about a risk. */
const DESC_PLACEHOLDER = 'What it is, why, what done looks like — Markdown'
const RECORD_TEXT = {
  story: { word: 'description', label: '', empty: 'No description', placeholder: DESC_PLACEHOLDER },
  task: { word: 'description', label: '', empty: 'No description', placeholder: DESC_PLACEHOLDER },
  question: { word: 'answer', label: 'Answer', empty: 'No answer yet', placeholder: 'The answer, and who gave it — Markdown' },
  decision: { word: 'decision', label: 'Decision', empty: 'Not written down', placeholder: 'What was decided, the options weighed, why this one — Markdown' },
  risk: { word: 'response', label: 'Response', empty: 'No response yet', placeholder: 'What we do about it: mitigation, fallback, who watches it — Markdown' },
}
const LEVEL_LABEL = { low: 'low', med: 'medium', high: 'high' }
function workOpenCount() {
  const w = S.pj.work
  if (!w || !w.nodes) return 0
  return Object.values(w.nodes).filter((n) => (n.kind === 'story' || n.kind === 'task') && !CLOSED_STATES.includes(n.state)).length
}
/** What waits for the user: suggested moves and merges, and "done" claims the verifier only half-believed. */
function workToReview(w) {
  const out = []
  for (const n of Object.values(w.nodes || {})) {
    for (const s of n.suggestions || []) if (s.status === 'pending') out.push({ n, s })
    const last = (n.history || []).filter((h) => h.type === 'agent' && h.effect).slice(-1)[0]
    if (last && (last.effect === 'done' || last.effect === 'held') && last.verdict === 'weak' && !CLOSED_STATES.includes(n.state)) out.push({ n, weak: last })
  }
  return out
}
const fmtDay = (d) => d ? new Date(d + 'T12:00').toLocaleDateString([], { weekday: d && (new Date(d + 'T12:00') - Date.now()) / 864e5 < 7 && (new Date(d + 'T12:00') - Date.now()) > -864e5 ? 'short' : undefined, day: 'numeric', month: 'short' }) : ''
/** A day as today / yesterday / its date, by the local calendar. `d`: a date (YYYY-MM-DD) or a whole timestamp
 *  (taken to its local day — never cut to 10 characters, which is the UTC day). */
const relDay = (d, now = new Date()) => {
  if (!d) return ''
  const at = String(d).length > 10 ? new Date(d) : null
  const day = at && !isNaN(at) ? ymd(at) : String(d).slice(0, 10)
  const t = new Date(now)
  if (day === ymd(t)) return 'today'
  t.setDate(t.getDate() - 1)
  return day === ymd(t) ? 'yesterday' : fmtDay(day)
}
/** A work item's status as the tree and the board show it: its word and its colour. */
function workStatus(n) {
  if (n.kind === 'question') return n.description || n.state === 'done' ? ['answered', 'done'] : CLOSED_STATES.includes(n.state) ? [STATE_LABEL[n.state] || n.state, n.state] : ['open', 'open']
  if (n.kind === 'risk') return CLOSED_STATES.includes(n.state) ? [STATE_LABEL[n.state] || n.state, n.state] : n.description ? ['handled', 'done'] : ['no response', 'waiting']
  if (n.state === 'done' && n.kind === 'task' && n.type === 'session') return ['held', 'done']
  // Blocked is the user's own word: it wins over waiting for someone, as on the board (`cardCategory`).
  if (n.waiting_for && n.state !== 'blocked' && !CLOSED_STATES.includes(n.state)) return ['waiting', 'waiting']
  return [STATE_LABEL[n.state] || n.state, n.state]
}
/** A card's category on the board: the column state it goes in. */
function cardCategory(n) {
  if (n.state === 'done') return 'done'
  if (n.state === 'blocked') return 'blocked'
  if (n.waiting_for) return 'waiting'
  return n.state === 'in_progress' ? 'in_progress' : 'open'
}
/** The Work bar's Owner options (value, label, chosen) for the items `all` and the view `V`. The user
 *  (`w.me`: their name, and the owners here that are them — "Me", their name) is one option, '@me', first. */
function workOwners(all, V, me) {
  const mine = me.names || []
  const isMe = V.owner === '@me' || mine.includes(V.owner)
  const owners = [...new Set(all.map((n) => n.internal_owner || n.client_owner).filter((o) => o && !mine.includes(o)))].sort()
  // A view saved on '@me' keeps its option even when no item here is the user's any more (its chip stays "Me").
  return [...(mine.length || V.owner === '@me' ? [['@me', me.name ? `Me (${me.name})` : 'Me', isMe]] : []), ...owners.map((o) => [o, o, V.owner === o])]
}
/** What the Work bar's filters keep: `pass` in the tree, `board` on the board. Done follows the Owner and
 *  Kind filters there too; only Open only leaves it alone (#113). */
function workFilter(V, me) {
  const mine = me.names || []
  const isMe = V.owner === '@me' || mine.includes(V.owner)
  const owner = (n) => n.internal_owner || n.client_owner
  const match = (n) => (!V.kind || n.kind === V.kind)
    && (!V.owner || (V.owner === '-' ? !owner(n) && n.kind === 'task' : isMe ? mine.includes(owner(n)) : owner(n) === V.owner))
  const open = (n) => !V.open || !CLOSED_STATES.includes(n.state)
  return { pass: (n) => open(n) && match(n), board: (n) => match(n) && (open(n) || n.state === 'done') }
}
/** A board card's due is late (by the local date), and it shows whom it waits on (not once done). */
const cardLate = (n, now = new Date()) => !!n.due && n.due < ymd(now) && n.state !== 'done'
const cardWaits = (n) => !!n.waiting_for && n.state !== 'done'
// end of the work-status block
/** Where a record came from: its day and the call ("today · weekly 12:05"). */
function workOrigin(w, n) {
  const h = (n.history || []).find((x) => x.op === 'create')
  const item = h && h.evidence && (w.evidence || {})[h.evidence]
  const call = item ? String(item.title || '').split(/[—–-]/)[0].trim().split(/\s+/).slice(0, 2).join(' ').toLowerCase() : ''
  return [n.kind === 'risk' ? `raised ${relDay(n.created)}` : relDay(n.created), call ? `${call}${h.ts ? ' ' + String(h.ts).replace(/^00:/, '').slice(0, 5) : ''}` : ''].filter(Boolean).join(' · ')
}
function workRow(w, n, level) {
  const kind = n.kind === 'task' ? (n.type !== 'action' ? KIND_LABEL[n.type] : '') : KIND_LABEL[n.kind] || ''
  const meta = n.kind === 'story' ? [n.ac.length ? `criteria ${n.ac.filter((a) => a.met).length} / ${n.ac.length}` : '', n.due ? `due ${fmtDay(n.due)}` : ''].filter(Boolean).join(' · ')
    : n.kind === 'task' ? [n.state === 'done' && n.type === 'session' ? `held ${relDay((n.history || []).filter((h) => h.effect === 'held').map((h) => h.date).pop() || n.created)}` : (n.internal_owner || n.client_owner || 'nobody'), n.state !== 'done' && n.due ? fmtDay(n.due) : ''].filter(Boolean).join(' · ')
    : workOrigin(w, n)
  const [word, cls] = workStatus(n)
  const st = n.kind === 'story' || n.kind === 'task' || n.kind === 'question' || n.kind === 'risk' ? `<span class="wk-st ${esc(cls)}">● ${esc(word)}</span>` : ''
  const tags = [n.pinned && n.pinned.length ? `<span class="tag">pinned: ${esc(n.pinned.join(', '))}</span>` : '', n.blocks && n.blocks.length && !CLOSED_STATES.includes(n.state) ? `<span class="tag">holds up ${n.blocks.length}</span>` : ''].join('')
  return `<button class="wk-row${S.pj.node === n.id ? ' on' : ''}${CLOSED_STATES.includes(n.state) ? ' closed' : ''}" style="padding-left:${12 + level * 18}px" data-pj="node" data-node="${esc(n.id)}"><span class="wk-id">${esc(n.id)}</span>${kind ? `<span class="wk-k ${esc(n.kind === 'task' ? n.type : n.kind)}">${esc(kind)}</span>` : ''}<span class="wk-t${n.kind === 'story' ? ' story' : ''}">${esc(n.title)}</span>${tags}<span class="grow"></span><span class="wk-meta">${esc(meta)}</span>${st}</button>`
}
/** Initials for an avatar ("Pat Novak" → "PN"), with a colour of its own. */
function avatar(name, owed) {
  if (!name) return '<span class="bd-av none" title="Nobody named">?</span>'
  const ini = name.split(/\s+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('')
  const hue = ProjectArt.hash(name) % 360
  return `<span class="bd-av${owed ? ' owed' : ''}" style="--h:${hue}" title="${esc(name)}${owed ? ' (the client owes it)' : ''}">${esc(ini)}</span>`
}
/** The board, as GitHub Projects lays one out: columns of a fixed width (the board scrolls sideways,
 *  cards are never squeezed), each a project's own Status option in a state category; cards with
 *  where they belong on top, the title, then the fields the project shows; optional lanes. */
const DEFAULT_COLUMNS = [
  { id: 'todo', name: 'To do', color: '#8a8a95', state: 'open' },
  { id: 'doing', name: 'In progress', color: '#9b78ff', state: 'in_progress' },
  { id: 'waiting', name: 'Waiting', color: '#f0a24a', state: 'waiting' },
  { id: 'blocked', name: 'Blocked', color: '#e8603c', state: 'blocked' },
  { id: 'done', name: 'Done', color: '#3ccf91', state: 'done' },
]
const CARD_FIELDS = [['story', 'Story'], ['kind', 'Kind'], ['owner', 'Owner'], ['due', 'Due date'], ['waiting', 'Waiting on'], ['holds_up', 'Holds up'], ['source', 'Last call']]
const DEFAULT_FIELDS = ['story', 'kind', 'owner', 'due', 'waiting', 'holds_up']
const STATE_CATS = [['open', 'To do'], ['in_progress', 'In progress'], ['waiting', 'Waiting on someone'], ['blocked', 'Blocked'], ['done', 'Done']]
const COL_COLORS = ['#8a8a95', '#9b78ff', '#5aa9ff', '#4cc3c8', '#3ccf91', '#c9b458', '#f0a24a', '#e8603c', '#d36bd1']
function boardOf() {
  const b = (S.pj.detail && S.pj.detail.board) || {}
  return { columns: b.columns && b.columns.length ? b.columns : DEFAULT_COLUMNS, fields: b.fields && b.fields.length ? b.fields : DEFAULT_FIELDS, group: b.group || 'none', views: b.views && b.views.length ? b.views : DEFAULT_VIEWS }
}
/** Saved views of the Work tab, as GitHub Projects keeps them: each its layout, filters, grouping,
 *  card fields and hidden columns; the columns themselves are the project's. */
const DEFAULT_VIEWS = [{ id: 'board', name: 'Board', layout: 'board' }, { id: 'tree', name: 'Tree', layout: 'tree' }]
const viewKey = () => `echo.pj.view.${S.pj.sel}`
function activeViewId() {
  const views = boardOf().views
  let id = S.pj.viewIds && S.pj.viewIds[S.pj.sel]
  if (!id) { try { id = localStorage.getItem(viewKey()) } catch { /* no storage */ } }
  return views.some((v) => v.id === id) ? id : views[0].id
}
/** The active view, its settings filled in. */
function wv() {
  const b = boardOf()
  const v = b.views.find((x) => x.id === activeViewId()) || b.views[0]
  return { id: v.id, name: v.name || 'View', layout: v.layout === 'tree' ? 'tree' : 'board', kind: v.kind || '', owner: v.owner || '', open: v.open !== false, group: v.group || b.group || 'none', fields: v.fields && v.fields.length ? v.fields : b.fields, hidden: v.hidden || [] }
}
function setView(patch) {
  const b = boardOf()
  const id = activeViewId()
  b.views = b.views.map((v) => v.id === id ? { ...v, ...patch } : v)
  saveBoard(b)
}
function chooseView(id) {
  S.pj.viewIds = { ...(S.pj.viewIds || {}), [S.pj.sel]: id }
  try { localStorage.setItem(viewKey(), id) } catch { /* no storage */ }
  S.pj.viewMenu = false
  render()
}
function viewTabs() {
  const b = boardOf(), cur = activeViewId()
  const icon = (v) => v.layout === 'tree' ? ICON('<path d="M5 5h6M5 12h6M5 19h6M14 5h5M14 12h5M14 19h5"/>') : ICON('<rect x="4" y="5" width="4.5" height="14" rx="1"/><rect x="10" y="5" width="4.5" height="9" rx="1"/><rect x="16" y="5" width="4" height="11" rx="1"/>')
  const tab = (v) => {
    const on = v.id === cur
    const menu = on && S.pj.viewMenu ? `<div class="menu bd-menu wv-menu" role="menu"><div class="bd-menu-row"><input data-view-name value="${esc(v.name || 'View')}" aria-label="View name" autofocus></div><button class="menu-item" data-pj="view-dup">Duplicate view</button>${b.views.length > 1 ? '<button class="menu-item danger" data-pj="view-del">Delete view</button>' : ''}</div>` : ''
    return `<span class="wv-tab${on ? ' on' : ''}"><button class="wv-b" data-pj="${on ? 'view-menu' : 'view-pick'}" data-v="${esc(v.id)}"${on ? ' aria-current="true"' : ''}${on ? ' title="Rename, duplicate or delete this view" aria-haspopup="menu"' : ''}>${icon(v)}${esc(v.name || 'View')}</button>${on ? `<span class="pj-add-wrap"><button class="icon-btn wv-more" data-pj="view-menu" aria-label="View options" aria-haspopup="menu" aria-expanded="${!!S.pj.viewMenu}">${ICON('<path d="M7 10l5 5 5-5"/>')}</button>${menu}</span>` : ''}</span>`
  }
  return `<div class="wv-tabs" role="group" aria-label="Views">${b.views.map(tab).join('')}<button class="wv-add" data-pj="view-new">+ New view</button></div>`
}

function saveBoard(b) {
  if (S.pj.detail) S.pj.detail.board = b
  send({ cmd: 'project', op: 'board', id: S.pj.sel, board: b })
  render()
}
/** The visible column a card sits in (the one the user put it in, if it still fits). */
function cardColumn(n, cols) {
  const cat = cardCategory(n)
  const own = cols.find((c) => c.id === n.column && c.state === cat)
  return own || cols.find((c) => c.state === cat) || (cat === 'waiting' ? cols.find((c) => c.state === 'open') : null)
}
function workBoard(w, pass) {
  const N = w.nodes || {}
  const b = { ...boardOf(), ...(() => { const v = wv(); return { group: v.group, fields: v.fields, hide: v.hidden } })() }
  const cols = b.columns.filter((c) => !c.hidden && !b.hide.includes(c.id))
  const show = new Set(b.fields)
  const items = Object.values(N).filter((n) => (n.kind === 'task' || n.kind === 'question') && n.state !== 'merged' && n.state !== 'deleted' && n.state !== 'cancelled' && pass(n))
  const placed = items.map((n) => [n, cardColumn(n, cols)]).filter(([, c]) => c)
  const card = (n) => {
    const story = N[n.parent] && N[n.parent].kind === 'story' ? N[n.parent] : null
    const kind = n.kind === 'question' ? 'question' : n.type !== 'action' ? n.type : ''
    const owner = n.internal_owner || n.client_owner
    const late = cardLate(n)
    const last = (n.history || []).filter((h) => h.evidence).pop()
    const item = last && (w.evidence || {})[last.evidence]
    const chips = [
      show.has('kind') && kind ? `<span class="wk-k ${esc(kind)}">${esc(kind)}</span>` : '',
      show.has('due') && n.due ? `<span class="bd-chip${late ? ' late' : ''}" title="${late ? 'Overdue' : 'Due'}">${ICON('<rect x="4" y="5" width="16" height="15" rx="2.5"/><path d="M4 10h16M9 3v4M15 3v4"/>')}${esc(fmtDay(n.due))}</span>` : '',
      show.has('waiting') && cardWaits(n) ? `<span class="bd-chip warn" title="${esc(n.waiting_for.what || '')}">⏳ ${esc(n.waiting_for.who || 'someone')}</span>` : '',
      show.has('holds_up') && (n.blocks || []).length && n.state !== 'done' ? `<span class="bd-chip" title="Holds up: ${esc(n.blocks.map((x) => (N[x] || {}).title || x).join('; '))}">holds up ${n.blocks.length}</span>` : '',
      show.has('source') && item ? `<span class="bd-chip" title="${esc(item.title || '')}">▮ ${esc(relDay(item.date))}</span>` : '',
    ].filter(Boolean).join('')
    return `<div class="bd-card${S.pj.node === n.id ? ' on' : ''}${n.state === 'done' ? ' done' : ''}" role="listitem" draggable="true" data-pj="node" data-node="${esc(n.id)}" data-drag="${esc(n.id)}" tabindex="0">
      <div class="bd-top"><span class="bd-ctx"><span class="wk-id">${esc(n.id)}</span>${show.has('story') && story && b.group !== 'story' ? ` · ${esc(story.title)}` : ''}</span>${show.has('owner') && n.kind === 'task' ? avatar(owner, !!n.client_owner && !n.internal_owner) : ''}</div>
      <div class="bd-t">${esc(n.title)}</div>${chips ? `<div class="bd-f">${chips}</div>` : ''}
    </div>`
  }
  // Lanes (Group by): none, each story, or each owner.
  let lanes
  if (b.group === 'story') {
    const keyOf = (n) => (N[n.parent] && N[n.parent].kind === 'story' ? n.parent : '')
    const keys = [...new Set(placed.map(([n]) => keyOf(n)))].sort((x, y) => (x === '') - (y === '') || x.localeCompare(y, undefined, { numeric: true }))
    lanes = keys.map((k) => ({ title: k ? `<button class="bd-lane-t" data-pj="node" data-node="${esc(k)}"><span class="wk-id">${esc(k)}</span><b>${esc(N[k].title)}</b></button>${N[k].ac.length ? `<span class="bd-prog"><span style="width:${Math.round(100 * N[k].ac.filter((a) => a.met).length / N[k].ac.length)}%"></span></span><span class="bd-meta">${N[k].ac.filter((a) => a.met).length}/${N[k].ac.length}</span>` : ''}` : '<b class="bd-lane-loose">Not in a story</b>', rows: placed.filter(([n]) => keyOf(n) === k) }))
  } else if (b.group === 'owner') {
    const keyOf = (n) => n.internal_owner || n.client_owner || ''
    const keys = [...new Set(placed.map(([n]) => keyOf(n)))].sort((x, y) => (x === '') - (y === '') || x.localeCompare(y))
    lanes = keys.map((k) => ({ title: k ? `${avatar(k)}<b>${esc(k)}</b>` : '<b class="bd-lane-loose">Nobody named</b>', rows: placed.filter(([n]) => keyOf(n) === k) }))
  } else lanes = [{ title: '', rows: placed }]
  const count = (c) => placed.filter(([, x]) => x.id === c.id).length
  const head = (c, i) => {
    const n = count(c), over = c.limit && n > c.limit
    const menu = S.pj.colMenu === c.id ? `<div class="menu bd-menu" role="menu">
        <div class="bd-menu-row"><input id="bd-col-name" data-col-name="${esc(c.id)}" value="${esc(c.name)}" aria-label="Column name"></div>
        <div class="bd-menu-row bd-colors">${COL_COLORS.map((x) => `<button class="pj-swatch${c.color === x ? ' on' : ''}" style="background:${x}" data-pj="col-set" data-col="${esc(c.id)}" data-k="color" data-v="${x}" aria-label="Colour ${x}"></button>`).join('')}</div>
        <label class="bd-menu-row">Cards in it are <select data-col-state="${esc(c.id)}">${STATE_CATS.map(([k, l]) => `<option value="${k}"${c.state === k ? ' selected' : ''}>${esc(l.toLowerCase())}</option>`).join('')}</select></label>
        <label class="bd-menu-row">Limit <input type="number" min="0" data-col-limit="${esc(c.id)}" value="${c.limit || ''}" placeholder="none" aria-label="Limit"></label>
        <div class="menu-sep" role="separator"></div>
        ${i > 0 ? `<button class="menu-item" data-pj="col-move" data-col="${esc(c.id)}" data-d="-1">Move left</button>` : ''}${i < cols.length - 1 ? `<button class="menu-item" data-pj="col-move" data-col="${esc(c.id)}" data-d="1">Move right</button>` : ''}
        <button class="menu-item" data-pj="col-set" data-col="${esc(c.id)}" data-k="hidden" data-v="true">Hide</button>
        <button class="menu-item danger" data-pj="col-delete" data-col="${esc(c.id)}">Delete column</button>
      </div>` : ''
    return `<div class="bd-col-h${n ? '' : ' empty'}" role="group" aria-label="${esc(c.name)}" title="${esc(c.name)}"><span class="bd-dot" style="background:${esc(c.color)}"></span><b class="bd-hl">${esc(c.name)}</b><span class="bd-n${over ? ' over' : ''}">${n}${c.limit ? ` / ${c.limit}` : ''}</span><span class="grow"></span><span class="pj-add-wrap"><button class="icon-btn bd-more" data-pj="col-menu" data-col="${esc(c.id)}" aria-label="Column options" title="Column options">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${menu}</span></div>`
  }
  const hiddenCols = b.columns.filter((c) => c.hidden || b.hide.includes(c.id))
  // Every column a card's width: the board scrolls sideways when there are many.
  const grid = `grid-template-columns: repeat(${cols.length}, var(--bd-w)) var(--bd-add)`
  const header = `<div class="bd-grid bd-heads" style="${grid}">${cols.map(head).join('')}<div class="bd-col-h add"><button class="tok add" data-pj="col-add">+ Column</button>${hiddenCols.length ? `<button class="link-btn" data-pj="col-unhide">${hiddenCols.length} hidden</button>` : ''}</div></div>`
  const laneHtml = (l) => `${l.title ? `<div class="bd-lane-h">${l.title}<span class="bd-meta">${l.rows.length}</span></div>` : ''}<div class="bd-grid" style="${grid}">${cols.map((c) => `<div class="bd-col" role="list" aria-label="${esc(c.name)}" data-drop="${c.state === 'waiting' ? '' : esc(c.id)}">${l.rows.filter(([, x]) => x.id === c.id).map(([n]) => card(n)).join('') || '<div class="bd-empty"></div>'}</div>`).join('')}<div></div></div>`
  if (!placed.length) return `<div class="wk-empty"><b>Nothing here</b><p>No tasks match the filters.</p></div>`
  // data-keep-scroll: a redraw (opening a column's menu) keeps where the board was scrolled sideways (#109).
  return `<div class="bd${b.group !== 'none' ? ' grouped' : ''}" data-keep-scroll>${header}${lanes.map(laneHtml).join('')}</div>`
}
// Drag a card to another column: its state, as the user set it (pinned, undoable).
document.addEventListener('dragstart', (e) => {
  const c = e.target.closest && e.target.closest('[data-drag]')
  if (!c) return
  e.dataTransfer.setData('text/x-echo-node', c.dataset.drag)
  e.dataTransfer.effectAllowed = 'move'
  c.classList.add('dragging')
})
document.addEventListener('dragend', (e) => { const c = e.target.closest && e.target.closest('[data-drag]'); if (c) c.classList.remove('dragging'); document.querySelectorAll('.bd-col.over').forEach((x) => x.classList.remove('over')) })
document.addEventListener('dragover', (e) => {
  const cell = e.target.closest && e.target.closest('.bd-col')
  if (!cell || !cell.dataset.drop || !e.dataTransfer.types.includes('text/x-echo-node')) return
  e.preventDefault()
  document.querySelectorAll('.bd-col.over').forEach((x) => x !== cell && x.classList.remove('over'))
  cell.classList.add('over')
})
document.addEventListener('drop', (e) => {
  const cell = e.target.closest && e.target.closest('.bd-col')
  const id = e.dataTransfer && e.dataTransfer.getData('text/x-echo-node')
  if (!cell || !cell.dataset.drop || !id) return
  e.preventDefault()
  cell.classList.remove('over')
  const n = S.pj.work && S.pj.work.nodes[id]
  const col = boardOf().columns.find((c) => c.id === cell.dataset.drop)
  if (!n || !col || n.column === col.id && cardCategory(n) === col.state) return
  const set = { column: col.id }
  if (cardCategory(n) !== col.state) set.state = col.state
  Object.assign(n, set) // shown at once; the ledger answers on the next load
  ledgerEdit({ op: 'edit', node: id, set })
  render()
})

function boardChips() {
  const b = wv()
  const groups = [['none', 'No grouping'], ['story', 'Story'], ['owner', 'Owner']]
  const group = `<label class="fchip${b.group !== 'none' ? ' on' : ''}">${b.group === 'none' ? 'Group' : `Group: ${esc((groups.find(([k]) => k === b.group) || [])[1])}`} ▾<select data-board-group aria-label="Group by">${groups.map(([k, l]) => `<option value="${k}"${b.group === k ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  const fields = `<span class="pj-add-wrap"><button class="fchip" data-pj="fields-menu">Fields ▾</button>${S.pj.fieldsMenu ? `<div class="menu bd-menu" role="menu">${CARD_FIELDS.map(([k, l]) => `<label class="menu-item bd-check"><input type="checkbox" data-board-field="${k}" ${b.fields.includes(k) ? 'checked' : ''}> ${esc(l)}</label>`).join('')}</div>` : ''}</span>`
  return group + fields
}
/** The rows of a work item's details that link it to other work: each a group named by its label. */
const XLINK_ROWS = ['Waits for', 'Holds up', 'Same as', 'Also in']
const KIND_FILTERS = [['', 'All kinds'], ['story', 'Stories'], ['task', 'Tasks'], ['decision', 'Decisions'], ['risk', 'Risks'], ['question', 'Questions']]
function projectWork(p) {
  const w = S.pj.work
  if (!w) return `<div class="skeleton">${'<i></i>'.repeat(8)}</div>`
  if (w.error) return `<p class="error">${esc(w.error)}</p>`
  const N = w.nodes || {}
  const all = Object.values(N).filter((n) => n.kind !== 'epic' && n.state !== 'merged' && n.state !== 'deleted')
  const sync = w.sync || {}
  const review = workToReview(w)
  const syncText = sync.state === 'running' ? `${esc((sync.message || 'reading calls').replace(/^reading/, 'Reading'))}…`
    : sync.state === 'failed' ? `<span class="error" title="${esc(sync.message || '')}">Sync failed</span>`
    : [sync.at ? `Synced ${esc(hm(new Date(sync.finished_at || sync.at)))}` : 'Not synced yet', w.pending ? `${w.pending} call${w.pending === 1 ? '' : 's'} to read` : '', review.length ? `<span class="wk-warn">${review.length} to review</span>` : ''].filter(Boolean).join(' · ')
  const counts = Object.fromEntries(KIND_FILTERS.map(([k]) => [k, all.filter((n) => !k || n.kind === k).length]))
  const V = wv()
  const owners = workOwners(all, V, w.me || {})
  const layoutChip = `<label class="fchip">${V.layout === 'tree' ? 'Tree' : 'Board'} ▾<select data-view-layout aria-label="Layout"><option value="board"${V.layout === 'board' ? ' selected' : ''}>Board</option><option value="tree"${V.layout === 'tree' ? ' selected' : ''}>Tree</option></select></label>`
  const kindChip = `<label class="fchip${V.kind ? ' on' : ''}">${esc((KIND_FILTERS.find(([k]) => k === V.kind) || KIND_FILTERS[0])[1])} ▾<select data-pj-filter="kind" aria-label="Kind">${KIND_FILTERS.map(([k, l]) => `<option value="${k}"${V.kind === k ? ' selected' : ''}>${esc(l)} · ${counts[k]}</option>`).join('')}</select></label>`
  const ownerChip = `<label class="fchip${V.owner ? ' on' : ''}">${esc(V.owner === '-' ? 'Nobody named' : (owners.find(([, , on]) => on) || [])[1] || V.owner || 'Owner')} ▾<select data-pj-filter="owner" aria-label="Owner"><option value="">Anyone</option><option value="-"${V.owner === '-' ? ' selected' : ''}>Nobody named</option>${owners.map(([v, l, on]) => `<option value="${esc(v)}"${on ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select></label>`
  const head = `${viewTabs()}<div class="wk-bar">${layoutChip}${kindChip}${ownerChip}<button class="fchip${V.open ? ' on' : ''}" data-pj="open-only" aria-pressed="${!!V.open}">Open only</button>${V.layout === 'board' ? boardChips() : ''}<span class="grow"></span><span class="wk-sync">${syncText}</span><button class="link-btn" data-pj="sync"${sync.state === 'running' ? ' disabled' : ''}>Sync now</button></div>`
  const { pass, board } = workFilter(V, w.me || {})
  if (!all.length) {
    return `${head}<div class="wk-empty"><b>No work yet</b><p>${w.pending ? `Sync reads this thread's calls — ${w.pending} so far — into stories, tasks, decisions, risks and questions, each with the words it came from.` : 'Once this thread has calls, Sync reads them into stories, tasks, decisions, risks and questions.'} It uses the thread's AI profile.</p><button class="btn primary" data-pj="sync">Sync now</button></div>`
  }
  const reviewList = review.length ? `<div class="wk-review">${review.map((r) => r.s
    ? `<div class="wk-rv" role="group" aria-label="To review"><span class="wk-warn">To review</span><span class="grow">${esc(r.s.op === 'suggest_merge' ? 'Same work as' : 'Belongs under')} <b>${esc((N[r.s.to] || {}).title || r.s.to)}</b>? <span class="set-sub">${esc(r.n.title)} — ${r.s.quote ? `“${esc(r.s.quote)}”` : esc(r.s.note || '')}</span></span><button class="btn primary" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'accept', target: r.s.mid }))}'>Yes</button><button class="btn ghost" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'reject', target: r.s.mid }))}'>No</button></div>`
    : `<div class="wk-rv" role="group" aria-label="To review"><span class="wk-warn">To review</span><span class="grow">Is <b>${esc(r.n.title)}</b> done? <span class="set-sub">“${esc(r.weak.quote || '')}”</span></span><button class="btn primary" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'edit', node: r.n.id, set: { state: 'done' } }))}'>Done</button><button class="btn ghost" data-pj="node" data-node="${esc(r.n.id)}">Look</button></div>`).join('')}</div>` : ''
  if (V.layout === 'board') return `${head}${reviewList}${workBoard(w, board)}`
  const rows = []
  const walk = (id, level) => {
    const n = N[id]
    if (!n || n.state === 'merged' || n.state === 'deleted') return
    const order = ['story', 'task', 'question', 'decision', 'risk']
    const kids = (n.children || []).filter((c) => N[c]).sort((a, b) => order.indexOf(N[a].kind) - order.indexOf(N[b].kind) || a.localeCompare(b))
    if (n.kind === 'epic') {
      const before = rows.length
      rows.push('')
      for (const c of kids) walk(c, 0)
      if (rows.length > before + 1) rows[before] = `<div class="wk-ws">${esc(n.title)}<span> · ${n.plan === 'Off-plan' ? 'the sync found work outside the plan' : 'workstream'}</span></div>`
      return
    }
    const below = rows.length
    // As designed: a story's tasks and questions sit under it; its decisions and risks at its own level.
    for (const c of kids) walk(c, N[c].kind === 'decision' || N[c].kind === 'risk' ? level : level + 1)
    const kidsShown = rows.length > below
    if (pass(n) || kidsShown) rows.splice(below, 0, workRow(w, n, level))
  }
  for (const r of w.roots || []) walk(r, 0)
  return `${head}${reviewList}<div class="wk-tree">${rows.join('') || '<p class="set-sub">Nothing matches.</p>'}</div>`
}

/** The selected work item, in the right panel (`work-item`): as the design's detail — a facts card
 *  read as text (click a value to change it), then its history with the words and the verifier's mark. */
function workPanel() {
  const w = S.pj.work, n = w && w.nodes && S.pj.node ? w.nodes[S.pj.node] : null
  if (!n) return '<div class="wp-empty">Pick a story, task or record on the Work tab.</div>'
  const N = w.nodes, ev = w.evidence || {}
  const kind = n.kind === 'task' ? (n.type === 'action' || !n.type ? 'task' : n.type) : n.kind
  const path = []
  for (let p = n.parent; p && N[p]; p = N[p].parent) path.unshift(N[p].title)
  const edit = S.pj.editField
  // A person's field (Owner, Handed over by, Decided by) suggests the thread's people as it is typed.
  const names = [...new Set(['Me', ...((projById(S.pj.sel) || {}).people || []).map((p) => p.name)].filter(Boolean))]
  const who = `list="wp-names"><datalist id="wp-names">${names.map((x) => `<option value="${esc(x)}">`).join('')}</datalist`
  // A value is named by its field too ("Handed over by: Dana Lee"): the grid's label beside it isn't its name.
  const FIELD = { state: 'Status', internal_owner: 'Owner', delegated_by: 'Handed over by', decided_by: 'Decided by', due: 'Due', how: 'How', likelihood: 'Likelihood', impact: 'Impact' }
  const val = (field, text, empty, input) => edit === field ? input
    : `<button class="wp-val${text ? '' : ' empty'}" data-pj="edit-field" data-field="${field}" aria-label="${esc(FIELD[field] || field)}: ${esc(text || `${empty} · Set`)}">${text ? esc(text) : `${esc(empty)} · <u>Set</u>`}</button>`
  const titles = (ids) => ids.map((i) => `<button class="wp-link" data-pj="node" data-node="${esc(i)}">${esc(i)} ${esc((N[i] || {}).title || '')}</button>`).join('')
  const trackable = n.kind === 'story' || n.kind === 'task' || n.kind === 'question'
  const [word, cls] = workStatus(n)
  const states = n.kind === 'question' ? [['open', 'open'], ['done', 'answered']] : [['open', 'to do'], ['in_progress', 'in progress'], ['blocked', 'blocked'], ['done', n.type === 'session' ? 'held' : 'done'], ['cancelled', 'cancelled']]
  const actionItems = (n.history || []).filter((h) => h.op === 'create' && h.quote).slice(0, 1)
  const rows = [
    trackable ? ['Status', edit === 'state' ? `<select class="wp-in" data-ledger-set="state" aria-label="Status">${states.map(([v, l]) => `<option value="${v}"${n.state === v ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>` : `<button class="wp-val" data-pj="edit-field" data-field="state"><span class="wk-st ${esc(cls)}">● ${esc(word)}</span></button>${n.derived ? ` <span class="set-sub">${esc(n.derived)}</span>` : ''}`] : null,
    n.kind === 'decision' ? ['Decided by', val('decided_by', n.decided_by, 'not known', `<input class="wp-in" data-ledger-set="decided_by" value="${esc(n.decided_by || '')}" placeholder="Me, or a name" aria-label="Decided by" autofocus ${who}>`)] : null,
    ...(n.kind === 'risk' ? [['likelihood', 'Likelihood'], ['impact', 'Impact']].map(([f, l]) => [l, val(f, LEVEL_LABEL[n[f]] || '', 'not rated', `<select class="wp-in" data-ledger-set="${f}" aria-label="${l}" autofocus><option value="">not rated</option>${Object.entries(LEVEL_LABEL).map(([v, t]) => `<option value="${v}"${n[f] === v ? ' selected' : ''}>${t}</option>`).join('')}</select>`)]) : []),
    n.kind === 'task' || n.kind === 'story' || n.kind === 'risk' ? ['Owner', val('internal_owner', n.internal_owner, 'nobody named', `<input class="wp-in" data-ledger-set="internal_owner" value="${esc(n.internal_owner || '')}" aria-label="Owner" autofocus ${who}>`)] : null,
    n.kind === 'task' ? ['Handed over by', val('delegated_by', n.delegated_by, n.creator ? `${n.creator} (said it)` : 'not known', `<input class="wp-in" data-ledger-set="delegated_by" value="${esc(n.delegated_by || '')}" placeholder="Me, or a name" aria-label="Handed over by" autofocus ${who}>`)] : null,
    n.client_owner ? ['Client owes', esc(n.client_owner)] : null,
    n.kind === 'task' || n.kind === 'story' ? ['Due', val('due', n.due ? new Date(n.due + 'T12:00').toLocaleDateString([], { weekday: 'short', day: 'numeric', month: 'short' }) : '', 'no date', `<input class="wp-in" type="date" data-ledger-set="due" value="${esc(n.due || '')}" aria-label="Due" autofocus>`) + ((n.due_history || []).length > 1 ? ` <span class="set-sub">moved ${n.due_history.length - 1}×</span>` : '')] : null,
    ...(n.kind === 'task' ? modeRows(S.pj.sel, n, val) : []),
    n.waiting_for ? ['Waiting on', `${esc(n.waiting_for.who || 'someone')} — ${esc(n.waiting_for.what || '')}`] : null,
    (n.waiting_on || []).length ? ['Waits for', titles(n.waiting_on)] : null,
    (n.blocks || []).length ? ['Holds up', titles(n.blocks)] : null,
    // Work in other threads (#20): the same work, or what it waits on / holds up there.
    n.mirror_of ? ['Same as', `${xItem(n.mirror_of.thread, n.mirror_of.node, n.mirror_of.title)} <span class="set-sub">— done there: its status, owner and due show here</span> ${xUnlink(n, n.mirror_of.thread, n.mirror_of.node)}`] : null,
    (n.also_in || []).length ? ['Also in', n.also_in.map((a) => `${xItem(a.thread, a.node, a.title)} ${xUnlink(n, a.thread, a.node)}`).join('<br>')] : null,
    (n.xwaits || []).length ? ['Waits for', n.xwaits.map((a) => `${xItem(a.thread, a.node, a.title)}${a.owner ? ` <span class="set-sub">· ${esc(a.owner)}</span>` : ''} ${xUnlink(n, a.thread, a.node)}`).join('<br>')] : null,
    ...(n.xrefs || []).filter((r) => r.kind === 'blocks').map((r) => ['Holds up', `${xItem(r.thread, r.node, '')} <span class="set-sub">in another thread</span> ${xUnlink(n, r.thread, r.node)}`]),
    (n.links || []).length ? ['From', n.links.filter((l) => /^(https|echo):\/\//i.test(l.url || '')).map((l) => /^echo:/i.test(l.url) ? `<a class="wp-link" href="${esc(l.url)}" title="In Echo">→ ${esc(l.title)}</a>` : `<a class="wp-link" href="${esc(l.url)}" data-ext title="${esc(l.url)}">↗ ${esc(l.title)}</a>`).join('')] : null,
    // Its own echo:// link: for a note, a mail, Knowledge — it opens here.
    S.pj.sel ? ['Link', `<button class="link-btn" data-copy="echo://task/${esc(S.pj.sel)}/${esc(n.id)}" title="echo://task/${esc(S.pj.sel)}/${esc(n.id)}">Copy link</button>`] : null,
    n.kind !== 'story' && actionItems.length ? ['Said', `“${esc(actionItems[0].quote.length > 90 ? actionItems[0].quote.slice(0, 88) + '…' : actionItems[0].quote)}”`] : null,
  ].filter(Boolean)
  // What the work is: an agent's from the words that created it, or yours (then it stays yours).
  // A record's: its answer, the decision, its response — under the title, edited the same way.
  const rt = RECORD_TEXT[n.kind]
  const de = S.pj.descEdit && S.pj.descEdit.node === n.id ? S.pj.descEdit : null
  const head = rt && rt.label ? `<div class="wp-h wp-rec-h">${esc(rt.label)}</div>` : ''
  const description = !rt ? ''
    : de ? `${head}<div class="wp-desc-edit"><textarea id="wp-desc" rows="6" spellcheck="true" aria-label="${esc(rt.label || 'Description')} (Markdown)" placeholder="${esc(rt.placeholder)}">${esc(de.text)}</textarea><div class="wp-desc-bar"><span class="set-sub">${MAC ? '⌘S' : 'Ctrl+S'} saves, Esc cancels</span><span class="grow"></span><button class="btn ghost" data-pj="desc-cancel">Cancel</button><button class="btn primary" data-pj="desc-save">Save</button></div></div>`
    : n.description ? `${head}<div class="wp-desc doc" data-pj="desc-edit" title="Click to change">${n.description_html || esc(n.description)}</div>`
    : `${head}<button class="wp-val empty wp-desc-add${n.kind === 'risk' && !CLOSED_STATES.includes(n.state) ? ' warn' : ''}" data-pj="desc-edit">${esc(rt.empty)} · <u>Add</u></button>`
  const met = n.ac.filter((a) => a.met).length
  const criteria = n.kind === 'story' ? `<div class="wp-h">Done when · ${met} of ${n.ac.length}</div><div class="wp-ac-list">${n.ac.map((a, i) => `<label class="wp-ac${a.met ? ' met' : ''}"><input type="checkbox" data-ledger-ac="${i + 1}" ${a.met ? 'checked' : ''}><span>${esc(a.text)}</span></label>`).join('')}</div>` : ''
  const hist = (n.history || []).slice().reverse().map((h) => {
    const what = h.by === 'human' && (h.op === 'xref' || h.op === 'unxref') ? `${h.op === 'xref' ? 'you linked it to' : 'you unlinked'} ${(projById(h.thread) || {}).name || h.thread} · ${h.target}${h.op === 'xref' ? (h.kind === 'same' ? (h.master === 'here' ? ' (the same work, done here)' : ' (the same work, done there)') : h.kind === 'depends_on' ? ' (waits on it)' : ' (it waits on this)') : ''}`
      : h.from ? `${h.effect || h.op} · heard in ${(projById(h.from.thread) || {}).name || h.from.thread}`
      : h.by === 'human' ? humanHistory(h, rt && rt.word, (id) => (N[id] && N[id].title) || '')
      : h.op === 'answer' ? `${n.kind === 'risk' ? 'response' : 'answered'}: ${h.text || ''}`
      : h.op === 'create' ? `created${h.due_text ? ' · due ' + h.due_text : ''}` : h.effect ? `${h.effect === 'due' ? 'due ' + (h.due_text || '') : h.effect}${h.effect === 'done' || h.effect === 'held' ? '?' : ''}`
      : h.op === 'waiting' ? `waiting · ${h.who || '?'}` : h.op === 'link' ? `waits for ${(N[h.depends_on] && N[h.depends_on].title) || 'another item'}` : h.op === 'ac_met' ? `criterion ${h.ac} met` : h.op
    const mark = h.by === 'human' ? (h.mid && h.op !== 'create' ? `<button class="link-btn" data-pj="ledger" data-body='${esc(JSON.stringify({ op: 'undo', target: h.mid }))}'>undo</button>` : '')
      : h.applied === false && h.verdict === 'supports' ? '<span class="wk-v weak">kept yours</span>'
      : h.verdict === 'supports' || (h.op === 'create') ? '<span class="wk-v supports">verified</span>'
      : h.verdict === 'weak' ? '<span class="wk-v weak">weak · to review</span>' : h.verdict === 'wrong' ? '<span class="wk-v wrong">wrong</span>'
      : (h.effect || ['ac_met', 'link', 'waiting', 'answer'].includes(h.op)) && h.effect !== 'mention' ? '<span class="wk-v">to check</span>' : ''
    const item = h.evidence && ev[h.evidence]
    // A call's words jump to the call; a mail's open it in Outlook.
    const srcs = !item ? '' : item.kind === 'mail'
      ? (/^https:\/\//i.test(item.url || '') ? `<a class="wp-src" href="${esc(item.url)}" data-ext title="${esc(item.title || 'The mail')} — open in Outlook">✉ ${esc(relDay(item.date))}</a>` : `<span class="wp-src" title="${esc(item.title || '')}">✉ ${esc(relDay(item.date))}</span>`)
      : `<button class="wp-src" data-call="${esc(item.call || '')}" title="${esc(item.title || '')}">▮ ${esc(h.ts ? String(h.ts).replace(/^00:/, '').slice(0, 5) : relDay(item.date))}</button>`
    return `<div class="wp-ev" role="listitem"><div class="wp-ev-top"><span class="set-sub">${esc(relDay(h.date))}</span><b>${esc(what)}</b>${h.speaker ? `<span class="set-sub">${esc(h.speaker)}</span>` : ''}<span class="grow"></span>${mark}</div>${h.quote ? `<div class="wp-q">“${esc(h.quote)}” ${srcs}</div>` : ''}</div>`
  }).join('')
  return `<div class="wp">
    <div class="wp-top"><span class="wk-id">${esc(n.id)}</span><span class="wk-k ${esc(n.kind === 'task' ? n.type : n.kind)}">${esc(kind)}</span><span class="grow"></span>${['task', 'story', 'question'].includes(n.kind) ? `<button class="btn ghost" data-claude-task="${esc(n.id)}" data-project="${esc(S.pj.sel)}" data-title="${esc(n.title)}"${modeData({ mode: n.who_does })} title="Send to Claude — work on it in the Claude panel, with a skill" aria-label="Send to Claude">Send to <span class="md-ico claude">${ICON(CLAUDE_MARK)}</span></button>` : ''}<button class="btn" data-pj="edit-title">Edit</button></div>
    ${edit === 'title' ? `<textarea class="wp-title" data-ledger-set="title" rows="1" aria-label="Title" autofocus>${esc(n.title)}</textarea>` : `<h3 class="wp-name">${esc(n.title)}</h3>`}
    ${path.length ? `<div class="set-sub">${esc(path.join(' › '))}</div>` : ''}
    ${description}
    ${rows.length ? `<div class="wp-facts">${rows.map(([k, v]) => `<span>${esc(k)}</span><div${XLINK_ROWS.includes(k) ? ` role="group" aria-label="${esc(k)}"` : ''}>${v}</div>`).join('')}</div>` : ''}
    ${criteria}
    ${['task', 'story', 'question'].includes(n.kind) ? xLinkBlock(n) : ''}
    ${(() => { const b = slotItems('task.actions', { kind: n.kind, node: n, project: S.pj.sel }); return b ? `<div class="acts plug-acts">${b}</div>` : '' })()}
    ${mentionsBlock(n)}
    <div class="wp-h">History</div><div class="wp-hist" role="list" aria-label="History">${hist}</div>
  </div>`
}
/** An item of another thread, as a link that opens it there (#20). */
function xItem(thread, node, title) {
  const p = projById(thread)
  return `<button class="wp-link" data-pj="goto-node" data-id="${esc(thread)}" data-node="${esc(node)}" title="Open it in ${esc(p ? p.name : thread)}">${esc(p ? p.name : thread)} · ${esc(node)}${title ? ' ' + esc(title) : ''}</button>`
}
const xUnlink = (n, thread, node) => `<button class="link-btn" data-pj="xunlink" data-node="${esc(n.id)}" data-thread="${esc(thread)}" data-target="${esc(node)}" title="Each is on its own again, with its own last values">Unlink</button>`
/** Link to an item in another thread (#20): a search over open work by its title (no model), then how they relate. */
function xLinkBlock(n) {
  const x = S.pj.xlink
  if (!x || x.node !== n.id) return `<button class="link-btn wp-xopen" data-pj="xlink-open" data-node="${esc(n.id)}">Link to an item in another thread…</button>`
  const rows = (x.results || []).map((r) => `<div class="wp-xr" role="group" aria-label="${esc(r.thread_name)} · ${esc(r.id)} ${esc(r.title)}"><div class="wp-xt"><b>${esc(r.thread_name)} · ${esc(r.id)}</b> ${esc(r.title)}${r.owner ? ` <span class="set-sub">· ${esc(r.owner)}</span>` : ''}</div>
      <div class="wp-xa"><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="same" data-master="there" title="One piece of work; ${esc(r.thread_name)} does it — this one shows its status">Same work · done there</button><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="same" data-master="here" title="One piece of work; this thread does it">Done here</button><button class="btn" data-pj="xlink" data-thread="${esc(r.thread)}" data-target="${esc(r.id)}" data-kind="depends_on" title="This one waits until that one is done">Waits on it</button></div></div>`).join('')
  return `<div class="wp-xlink"><div class="wp-h">Link to an item in another thread</div>
    <input class="wp-in" id="wp-xq" value="${esc(x.q || '')}" placeholder="Words from its title, or its id" aria-label="Find an item in another thread" autofocus>
    ${x.q && !(x.results || []).length ? '<p class="set-sub">No open item matches in your other threads.</p>' : rows}
    <button class="link-btn" data-pj="xlink-close">Cancel</button></div>`
}
let xlinkTimer = null
document.addEventListener('input', (e) => {
  if (e.target.id !== 'wp-xq' || !S.pj.xlink) return
  S.pj.xlink.q = e.target.value
  clearTimeout(xlinkTimer)
  xlinkTimer = setTimeout(async () => {
    const x = S.pj.xlink
    if (!x) return
    try { x.results = await get(`api/work/search?q=${encodeURIComponent(x.q)}&not=${encodeURIComponent(S.pj.sel || '')}`) } catch { x.results = [] }
    render()
    const i = $('#wp-xq'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length) }
  }, 250)
})
PANEL_VIEWS.push({ id: 'work-item', label: 'Details', icon: ICON('<rect x="4" y="4" width="16" height="16" rx="3"/><path d="M8.5 12.5l2.5 2.5 4.5-5"/>'), area: 'right', when: () => ((S.view === 'projects' && S.pj.tab === 'work') || S.pj.inline === S.view) && !!S.pj.node, render: workPanel })
function ledgerEdit(body, project = S.pj.sel) {
  send({ cmd: 'project', op: 'ledger', id: project, body })
  setTimeout(async () => {
    if (S.view === 'projects') await loadProject()
    else {
      if (S.pj.work && S.pj.sel === project) await loadWork()
      // Today's cards carry the task's mode (its ✦ the prompt): read again too.
      if (S.view === 'today') await loadToday()
      else if (S.view === 'tasks') await loadTasks()
      else if (S.selected) await loadCall(S.selected)
    }
    render()
  }, 300)
}

// ── column menu: where a board column's menu goes (no DOM: `column-menu.test.mjs`) ─────────────
/** A column's menu is placed against the window, not in the board: the board scrolls sideways, so its box
 *  clips what hangs out of it, and on a short board that was most of the menu (#109). Under its ⋯ button,
 *  right edges lined up; above it when there is no room below; kept inside the window. `b` the button's
 *  box, `m` the menu's size, `win` the window's. */
function colMenuSpot(b, m, win, pad = 8) {
  const below = b.bottom + 4
  const top = below + m.height <= win.height - pad ? below : Math.max(pad, Math.min(b.top - 4 - m.height, win.height - pad - m.height))
  const left = Math.max(pad, Math.min(b.right - m.width, win.width - pad - m.width))
  return { top, left }
}
// end of the column menu block
/** The open column menu (fixed, `.bd-col-h .bd-menu`) where `colMenuSpot` says: after each render, and
 *  when the board or the pane scrolls or the window is resized; hidden while its ⋯ is out of the board's view. */
function placeColMenu() {
  const m = document.querySelector('.bd-col-h .bd-menu'), btn = m && m.parentElement.querySelector('.bd-more')
  if (!btn) return
  // Its ⋯ out of the board's view (or the window's): the menu hides rather than float away from its column, and shows
  // again with it (no redraw: a redraw while the board is being scrolled to the column would close it at once).
  const b = btn.getBoundingClientRect(), board = btn.closest('.bd'), v = board ? board.getBoundingClientRect() : { left: 0, top: 0, right: window.innerWidth, bottom: window.innerHeight }
  const cx = (b.left + b.right) / 2, cy = (b.top + b.bottom) / 2
  const out = cx < Math.max(0, v.left) || cx > Math.min(window.innerWidth, v.right) || cy < Math.max(0, v.top) || cy > Math.min(window.innerHeight, v.bottom)
  m.style.visibility = out ? 'hidden' : ''
  const at = colMenuSpot(b, { width: m.offsetWidth, height: m.offsetHeight }, { width: window.innerWidth, height: window.innerHeight })
  m.style.top = `${at.top}px`
  m.style.left = `${at.left}px`
}
document.addEventListener('scroll', placeColMenu, { capture: true, passive: true })
window.addEventListener('resize', placeColMenu)

// ── human history: what the user did to an item, in words (History in its details) ─────────────
/** One of the user's own ledger edits as History says it: every op in words, none shown raw (#153). `word`: what
 *  the item's description is called (its answer, the decision…), for an edit of it. No DOM: `ui/human-history.test.mjs`. */
function humanHistory(h, word, title = () => '') {
  const item = (id) => (id && title(id)) || 'another item'
  const crit = (h.ac != null ? ` ${h.ac}` : '') + (h.ac_text ? ` (${h.ac_text})` : '')
  switch (h.op) {
    case 'edit': return `you set ${Object.keys(h.set || {}).map((k) => k === 'description' && word ? word : k.replace('_', ' ')).join(', ')}`
    case 'create': return 'you created it'
    case 'ac_add': return `you added a criterion${h.text ? `: ${h.text}` : ''}`
    case 'ac_set': {
      if (h.drop) return `you dropped criterion${crit}`
      const parts = [h.text ? `reworded criterion${crit}: ${h.text}` : '', h.met === true ? `ticked criterion${h.text ? '' : crit}` : h.met === false ? `unticked criterion${h.text ? '' : crit}` : '']
      const said = parts.filter(Boolean)
      return said.length ? `you ${said.join(' and ')}` : `you changed criterion${crit}`
    }
    case 'link': return `you made it wait for ${item(h.depends_on)}`
    case 'unlink': return `you took away its wait for ${item(h.depends_on)}`
    case 'merge': return `you merged it into ${item(h.into)}`
    case 'delete': return 'you deleted it'
    default: return 'you changed it'
  }
}
// end of the human history block
