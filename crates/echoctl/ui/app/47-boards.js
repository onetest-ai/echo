// ── A thread's and a client's own dashboards (`boards.rs`): tabs after the Brief (the Overview, which stays the first
// and the page's own), each on every thread (every client) with its layout shared and its widgets showing the page
// they are on. + adds one; its ⋯ renames it, moves it left or right, removes it.

async function loadBoards() {
  try { S.boards = await get('api/boards') } catch { /* app restarting */ }
}
const boardsOf = (kind) => ((S.boards || {})[kind]) || []
/** The board a tab id names (`b-<id>`), or null. */
const boardOfTab = (kind, tab) => (typeof tab === 'string' && tab.startsWith('b-') ? boardsOf(kind).find((b) => b.id === tab.slice(2)) || null : null)
/** The tabs with the boards after the first (the Brief, the Overview). */
const withBoards = (kind, tabs) => [tabs[0], ...boardsOf(kind).map((b) => [`b-${b.id}`, esc(b.name)]), ...tabs.slice(1)]
const KIND_WORD = { thread: 'thread', client: 'client' }

/** The + after the boards' tabs: a name field while adding. */
function boardPlus(kind) {
  if (S.boardNew === kind) return `<span class="bd-new"><input id="bd-new" data-board-new="${kind}" placeholder="Dashboard name" aria-label="New dashboard name" maxlength="40"></span>`
  return `<button class="tab bd-plus" data-board-add="${kind}" aria-label="New dashboard" title="A dashboard of its own, on every ${KIND_WORD[kind]}">+</button>`
}
/** The tab row's buttons, + after the last board. */
function boardTabRow(kind, tabs, active, attr) {
  const n = 1 + boardsOf(kind).length
  return tabs.map(([t, label], i) => `<button class="tab" role="tab" data-pj="${attr}" data-tab-id="${t}" aria-selected="${active === t}">${label}</button>${i === n - 1 ? boardPlus(kind) : ''}`).join('')
}

/** A board's page: its name with ⋯ (rename, move, remove), then its widgets. */
function boardPage(kind, b) {
  const all = boardsOf(kind), at = all.findIndex((x) => x.id === b.id)
  const name = S.boardRename === b.id
    ? `<input id="bd-rename" class="bd-rename" data-board-rename="${esc(b.id)}" value="${esc(b.name)}" aria-label="Dashboard name" maxlength="40">`
    : `<h3>${esc(b.name)}</h3>`
  const menu = `<span class="pj-add-wrap pj-more"><button class="icon-btn" data-board-menu="1" aria-haspopup="menu" aria-expanded="${!!S.boardMenu}" aria-label="Dashboard actions" title="Dashboard actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.boardMenu ? `<div class="menu" role="menu"><button class="menu-item" role="menuitem" data-board-do="rename">Rename</button><button class="menu-item" role="menuitem" data-board-do="left"${at <= 0 ? ' disabled' : ''}>Move left</button><button class="menu-item" role="menuitem" data-board-do="right"${at >= all.length - 1 ? ' disabled' : ''}>Move right</button><div class="menu-sep" role="separator"></div><button class="menu-item danger" role="menuitem" data-board-do="remove-ask">Remove…</button></div>` : ''}</span>`
  const ask = S.boardRemove === b.id ? `<div class="notice"><span class="grow">Remove <b>${esc(b.name)}</b> from every ${KIND_WORD[kind]}? Its widgets go with it.</span><button class="btn ghost" data-board-do="remove-cancel">Cancel</button><button class="btn danger" data-board-do="remove">Remove</button></div>` : ''
  return `<div class="bd-head">${name}<span class="set-sub">On every ${KIND_WORD[kind]}, each showing its own</span>${menu}</div>${ask}${dashboardView()}`
}

/** Send a board operation, then read the boards (and the page) again; `then` with them. */
function boardOp(body, then) {
  send({ cmd: 'board_op', ...body })
  setTimeout(async () => { await loadBoards(); if (then) then(); await loadDashboard(); render() }, 300)
}
const boardHere = () => {
  if (S.view !== 'projects') return null
  const kind = S.pj.client ? 'client' : 'thread'
  const b = boardOfTab(kind, S.pj.client ? S.pj.ctab : S.pj.tab)
  return b ? { kind, b } : null
}
const boardGo = (kind, id) => go(kind === 'client' ? `client/${S.pj.client}/${id ? `b-${id}` : 'overview'}` : `project/${S.pj.sel}/${id ? `b-${id}` : 'brief'}`)

document.addEventListener('click', (e) => {
  if (!e.target.closest) return
  const add = e.target.closest('[data-board-add]')
  if (add) { S.boardNew = add.dataset.boardAdd; render(); const i = $('#bd-new'); if (i) i.focus(); return }
  if (e.target.closest('[data-board-menu]')) { S.boardMenu = !S.boardMenu; return render() }
  const d = e.target.closest('[data-board-do]')
  const here = boardHere()
  if (!d || !here) { if (S.boardMenu && !e.target.closest('.menu')) { S.boardMenu = false; render() } return }
  const { kind, b } = here, all = boardsOf(kind), at = all.findIndex((x) => x.id === b.id)
  S.boardMenu = false
  switch (d.dataset.boardDo) {
    case 'rename': S.boardRename = b.id; render(); { const i = $('#bd-rename'); if (i) { i.focus(); i.select() } } return
    case 'left': return boardOp({ op: 'move', kind, id: b.id, to: Math.max(0, at - 1) })
    case 'right': return boardOp({ op: 'move', kind, id: b.id, to: at + 1 })
    case 'remove-ask': S.boardRemove = b.id; return render()
    case 'remove-cancel': S.boardRemove = null; return render()
    case 'remove': S.boardRemove = null; return boardOp({ op: 'remove', kind, id: b.id }, () => boardGo(kind, null))
  }
})

document.addEventListener('keydown', (e) => {
  const t = e.target
  if (!t || !t.dataset) return
  if (t.dataset.boardNew) {
    if (e.key === 'Escape') { S.boardNew = null; return render() }
    if (e.key !== 'Enter') return
    const kind = t.dataset.boardNew, name = t.value.trim()
    if (!name) return
    S.boardNew = null
    // The new one opens (found by its name: its id is the app's).
    boardOp({ op: 'add', kind, name }, () => { const n = boardsOf(kind).find((b) => b.name.toLowerCase() === name.toLowerCase()); if (n) boardGo(kind, n.id) })
  }
  if (t.dataset.boardRename) {
    if (e.key === 'Escape') { S.boardRename = null; return render() }
    if (e.key !== 'Enter') return
    const here = boardHere()
    S.boardRename = null
    if (here && t.value.trim() && t.value.trim() !== here.b.name) boardOp({ op: 'rename', kind: here.kind, id: here.b.id, name: t.value.trim() })
    else render()
  }
})
// end of the boards block
