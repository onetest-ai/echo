// The test plugin's window half (tests/cases/plugins/TC-180 to TC-183), its look its own:
// - on a call's header, a chip "Filed" / "Not filed" with its own popup to file the call in a vault (Acme's calls in
//   "Acme notes", the rest in "Inbox notes"), and a second box that always throws (the header draws without it);
// - in the call's ⋯ menu, "Open in <vault>" (a label made from the call) opening a sheet, and "About Call demo";
// - in the minutes box while the minutes are coming, a line naming the vault;
// - an activity "Filed calls": its badge the number of calls filed ('!' while none), a box whose text Echo leaves alone
//   while the user types in it, though the plugin asks for a redraw every second.
export function apply(ctx) {
  const esc = ctx.ui.esc
  const filed = {}
  let pop = null, shown = null, count = null, ticks = 0, sheetCall = null
  const clientOf = (c) => c.client !== undefined ? c.client : ((c.projects || []).find((p) => p.client) || {}).client
  const vault = (c) => clientOf(c) === 'acme' ? 'Acme notes' : 'Inbox notes'
  const recount = () => ctx.call('count').then((n) => { count = n; ctx.render() })
  const file = (name, on) => ctx.call('file', { call: name, on }).then((v) => { filed[name] = v; pop = null; recount() })
  recount()

  ctx.slots.add('call.header', {
    id: 'filed',
    label: 'Filed',
    render: (c) => {
      shown = c
      const f = filed[c.name]
      const chip = `<button class="btn ghost cd-chip" data-cd-chip aria-haspopup="menu" aria-expanded="${pop === c.name}">${f === undefined ? 'Reading…' : f ? '▤ Filed' : '▢ Not filed'}</button>`
      const menu = pop === c.name ? `<div class="menu cd-pop" role="menu"><button class="menu-item" role="menuitem" data-cd-toggle>${f ? 'Take it out of' : 'File it in'} ${esc(vault(c))}</button></div>` : ''
      return chip + menu
    },
    after: (box, c) => {
      if (filed[c.name] === undefined) { filed[c.name] = undefined; ctx.call('get', { call: c.name }).then((v) => { filed[c.name] = v; ctx.render() }) }
    },
  })
  ctx.slots.add('call.header', { id: 'broken', label: 'Broken', render: () => { throw new Error('call-demo breaks here on purpose') } })

  const openSheet = (c) => {
    sheetCall = c.name
    ctx.ui.sheet(`<h3>File “${esc(c.title)}”</h3><p>Into ${esc(vault(c))}. Recall is ${ctx.where().recall ? 'on' : 'off'}.</p>
      <input class="set-in" data-cd-note aria-label="A note for the vault" placeholder="A note for the vault">
      <div class="td-dialog-a"><button class="btn ghost" data-cd-cancel>Cancel</button><button class="btn primary" data-cd-file>File it</button></div>`,
    { label: 'File this call', onClose: () => { sheetCall = null; ctx.call('closed') } })
  }
  ctx.slots.add('call.menu', { id: 'open', label: (t) => `Open in ${vault(t.call)}`, run: (t) => openSheet(t.call) })
  ctx.slots.add('call.menu', { id: 'about', label: 'About Call demo', run: () => ctx.ui.notice('Call demo files your calls in your notes.') })
  ctx.slots.add('call.minutes', { id: 'line', label: 'Call demo', render: (c) => `Call demo files these minutes in <b>${esc(vault(c))}</b> once they are written.` })

  ctx.listen('click', (e) => {
    const t = e.target.closest ? e.target : null
    if (!t) return
    if (t.closest('[data-cd-chip]') && shown) { pop = pop === shown.name ? null : shown.name; return ctx.render() }
    if (t.closest('[data-cd-toggle]') && shown) return file(shown.name, !filed[shown.name])
    if (t.closest('[data-cd-cancel]')) return ctx.ui.closeSheet()
    if (t.closest('[data-cd-file]') && sheetCall) { const name = sheetCall; ctx.ui.closeSheet(); return file(name, true) }
    if (pop && !t.closest('.cd-pop')) { pop = null; ctx.render() }
  })

  // The activity: a redraw asked every second (its line "Drawn N times" grows), held while the user types in its box.
  const timer = setInterval(() => { ticks++; ctx.render() }, 1000)
  ctx.onUnload(() => clearInterval(timer))
  const typing = () => { const a = document.activeElement; return !!(a && a.matches && a.matches('[data-cd-draft]') && a.value !== '') }
  ctx.activities.add({
    id: 'filed',
    label: 'Filed calls',
    icon: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><path d="M4 6h16v13H4zM4 10h16M9 14h6"/></svg>',
    badge: () => count === null ? '' : count || '!',
    editing: typing,
    render: () => `<div class="set-main"><h2>Filed calls</h2><p>${count === null ? 'Reading…' : `${count} filed`} · Drawn ${ticks} times</p>
      <input class="set-in" data-cd-draft aria-label="A draft" placeholder="Type here: Echo leaves it alone"></div>`,
  })
}
