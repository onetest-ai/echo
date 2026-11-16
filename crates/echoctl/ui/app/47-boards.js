// ── A thread's and a client's own dashboards (`boards.rs`): tabs after the Brief (the Overview, which stays the first
// and the page's own), each on every thread (every client) with its layout shared and its widgets showing the page
// they are on. + adds one; its ⋯ renames it, moves it left or right, removes it.

// ── board-presets: which dashboards a thread shows, what the "+" offers, a preset dashboard's line (no DOM:
//    board-presets.test.mjs)
/** Whether board `b` shows on a thread of stage group `group` (its `for`; none: every thread). */
const boardShows = (b, group) => !(b.for || []).length || (b.for || []).includes(group)
/** What a kind's "+" offers from plugins: their presets not added yet, by name. */
const presetOffers = (presets) => (presets || []).filter((p) => !p.added).sort((a, b) => a.name.localeCompare(b.name) || a.plugin_name.localeCompare(b.plugin_name))
/** A preset dashboard's line under its name, from its `status`: {text, acts: [[do, label, primary]]} or null. */
function presetNote(st) {
  if (!st) return null
  const n = st.plugin_name
  if (st.state === 'update') return { text: `${n} updated this dashboard${st.edited ? ' — Update replaces your changes' : ''}`, acts: [['preset-keep', 'Keep mine', false], ['preset-update', 'Update', true]] }
  if (st.state === 'gone') return { text: 'Its plugin was removed: its widgets show nothing', acts: [['remove-ask', 'Remove…', false]] }
  if (st.state === 'off') return { text: `Its plugin, ${n}, is off: its widgets show nothing`, acts: [['remove-ask', 'Remove…', false]] }
  return null
}
// end of the board-presets block

async function loadBoards() {
  const was = JSON.stringify(S.boards || null)
  try { S.boards = await get('api/boards') } catch { return /* app restarting */ }
  // One added, renamed, moved or removed elsewhere: the tabs (and a removed one's page, back to the first) follow.
  if (was !== 'null' && JSON.stringify(S.boards) !== was) { await loadDashboard(); render() }
}
const boardsOf = (kind) => ((S.boards || {})[kind]) || []
/** The boards this page shows: a thread's only those for its stage group. */
const boardsShown = (kind) => {
  if (kind !== 'thread') return boardsOf(kind)
  const p = typeof projById === 'function' ? projById(S.pj.sel) : null
  const group = p ? ProjectArt.stage(p.stage).gid : ''
  return boardsOf(kind).filter((b) => boardShows(b, group))
}
/** The board a tab id names (`b-<id>`) and this page shows, or null. */
const boardOfTab = (kind, tab) => (typeof tab === 'string' && tab.startsWith('b-') ? boardsShown(kind).find((b) => b.id === tab.slice(2)) || null : null)
/** The tabs with the boards after the first (the Brief, the Overview). */
const withBoards = (kind, tabs) => [tabs[0], ...boardsShown(kind).map((b) => [`b-${b.id}`, esc(b.name)]), ...tabs.slice(1)]
const KIND_WORD = { thread: 'thread', client: 'client' }

/** The + after the boards' tabs: a name field while adding, and under it what the plugins offer ready-made. */
function boardPlus(kind) {
  const offers = presetOffers(((S.boards || {}).presets || {})[kind])
  const menu = offers.length ? `<div class="menu bd-presets" role="menu" aria-label="From plugins"><div class="menu-h">From plugins</div>${offers.map((p) => `<button class="menu-item" role="menuitem" data-board-preset="${esc(p.plugin)}" data-preset="${esc(p.preset)}" data-kind="${kind}" title="${esc(p.about || '')}">${esc(p.name)} <span class="set-sub">— ${esc(p.plugin_name)}</span></button>`).join('')}</div>` : ''
  if (S.boardNew === kind) return `<span class="bd-new pj-add-wrap"><input id="bd-new" data-board-new="${kind}" placeholder="Dashboard name" aria-label="New dashboard name" maxlength="40">${menu}</span>`
  return `<button class="tab bd-plus" data-board-add="${kind}" aria-label="New dashboard" title="A dashboard of its own, on every ${KIND_WORD[kind]}">+</button>`
}
/** The tab row's buttons, + after the last board. */
function boardTabRow(kind, tabs, active, attr) {
  const n = 1 + boardsShown(kind).length
  return tabs.map(([t, label], i) => `<button class="tab" role="tab" data-pj="${attr}" data-tab-id="${esc(t)}" aria-selected="${active === t}">${label}</button>${i === n - 1 ? boardPlus(kind) : ''}`).join('')
}

/** A board's page: its name with ⋯ (rename, move, remove) and ✎, then its widgets. */
function boardPage(kind, b) {
  const all = boardsShown(kind), at = all.findIndex((x) => x.id === b.id)
  const name = S.boardRename === b.id
    ? `<input id="bd-rename" class="bd-rename" data-board-rename="${esc(b.id)}" value="${esc(b.name)}" aria-label="Dashboard name" maxlength="40">`
    : `<h3>${esc(b.name)}</h3>`
  const menu = `<span class="pj-add-wrap pj-more"><button class="icon-btn" data-board-menu="1" aria-haspopup="menu" aria-expanded="${!!S.boardMenu}" aria-label="Dashboard actions" title="Dashboard actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.boardMenu ? `<div class="menu" role="menu"><button class="menu-item" role="menuitem" data-board-do="rename">Rename</button><button class="menu-item" role="menuitem" data-board-do="left"${at <= 0 ? ' disabled' : ''}>Move left</button><button class="menu-item" role="menuitem" data-board-do="right"${at >= all.length - 1 ? ' disabled' : ''}>Move right</button><div class="menu-sep" role="separator"></div><button class="menu-item danger" role="menuitem" data-board-do="remove-ask">Remove…</button></div>` : ''}</span>`
  const ask = S.boardRemove === b.id ? `<div class="notice"><span class="grow">Remove <b>${esc(b.name)}</b> from every ${KIND_WORD[kind]}? Its widgets go with it.</span><button class="btn ghost" data-board-do="remove-cancel">Cancel</button><button class="btn danger" data-board-do="remove">Remove</button></div>` : ''
  // A plugin's preset: a quiet line when its plugin changed it, is off or went.
  const note = presetNote(b.status)
  const line = note ? `<div class="bd-note" role="group" aria-label="From its plugin"><span class="set-sub">${esc(note.text)}</span>${note.acts.map(([d, label, primary]) => `<button class="btn ${primary ? 'primary' : 'ghost'} sm" data-board-do="${d}">${esc(label)}</button>`).join('')}</div>` : ''
  // On one line with the dashboard's ✎ (its own line until the dashboard is read).
  const head = `${name}<span class="set-sub">On every ${KIND_WORD[kind]}, each showing its own</span>${menu}${ask}${line}`
  return dashboardView(head) || `<div class="bdh">${head}</div>`
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
  // A plugin's preset: added, and opened (found by where it came from: its id is the app's).
  const pre = e.target.closest('[data-board-preset]')
  if (pre) {
    const kind = pre.dataset.kind, plugin = pre.dataset.boardPreset, preset = pre.dataset.preset
    S.boardNew = null
    // Opened here only if this page shows it (a pursuit thread's board added on a delivery thread is on the others).
    return boardOp({ op: 'add_preset', kind, plugin, preset }, () => { const n = boardsShown(kind).find((b) => b.from && b.from.plugin === plugin && b.from.preset === preset); if (n) boardGo(kind, n.id) })
  }
  const add = e.target.closest('[data-board-add]')
  if (add) { S.boardNew = add.dataset.boardAdd; render(); const i = $('#bd-new'); if (i) i.focus(); return }
  if (e.target.closest('[data-board-menu]')) { S.boardMenu = !S.boardMenu; return render() }
  const d = e.target.closest('[data-board-do]')
  const here = boardHere()
  if (!d || !here) { if (S.boardMenu && !e.target.closest('.menu')) { S.boardMenu = false; render() } return }
  // Left / right: past the next board this page shows (one for other stages stays where it is between them).
  const { kind, b } = here, all = boardsOf(kind), shown = boardsShown(kind), at = shown.findIndex((x) => x.id === b.id)
  const to = (n) => all.findIndex((x) => x.id === (shown[n] || {}).id)
  S.boardMenu = false
  switch (d.dataset.boardDo) {
    case 'rename': S.boardRename = b.id; render(); { const i = $('#bd-rename'); if (i) { i.focus(); i.select() } } return
    case 'left': return at > 0 ? boardOp({ op: 'move', kind, id: b.id, to: to(at - 1) }) : undefined
    case 'right': return at < shown.length - 1 ? boardOp({ op: 'move', kind, id: b.id, to: to(at + 1) }) : undefined
    case 'remove-ask': S.boardRemove = b.id; return render()
    case 'remove-cancel': S.boardRemove = null; return render()
    case 'remove': S.boardRemove = null; return boardOp({ op: 'remove', kind, id: b.id }, () => boardGo(kind, null))
    case 'preset-update': return boardOp({ op: 'preset_update', kind, id: b.id })
    case 'preset-keep': return boardOp({ op: 'preset_keep', kind, id: b.id })
  }
})

// Another page: an unfinished name, a menu or a question goes.
window.addEventListener('hashchange', () => { S.boardNew = null; S.boardRename = null; S.boardMenu = false; S.boardRemove = null })
// Clicked away from a name field: it closes (Enter keeps it, Esc drops it).
document.addEventListener('focusout', (e) => {
  const t = e.target
  if (!t || !t.dataset || !(t.dataset.boardNew || t.dataset.boardRename || t.dataset.boardPreset)) return
  // Into the "+"'s From plugins (Tab to a row): it stays open.
  if (e.relatedTarget && e.relatedTarget.closest && e.relatedTarget.closest('.bd-new')) return
  setTimeout(() => { const a = document.activeElement; if (a === t || (a && a.closest && a.closest('.bd-new'))) return; S.boardNew = null; S.boardRename = null; render() }, 150)
})
// A press on a From plugins row keeps the name field's focus: a slow click isn't lost to the field closing.
document.addEventListener('mousedown', (e) => { if (e.target.closest && e.target.closest('[data-board-preset]')) e.preventDefault() })

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
