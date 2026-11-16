// ── Settings → Plugins: installed, offered by the marketplaces, the marketplaces themselves. Every change
// runs in the app (IPC `plugin_op`); installs and fetches in the background, shown from `busy`.
async function loadPluginsView() {
  try { S.plugins = await get('api/plugins'); S.pluginsAt = Date.now() } catch { /* app restarting */ }
  const p = S.plugins
  // What is installed and on may have changed: the window's plugin parts follow.
  if (p && !p.error) loadPluginClients(p)
  // While something runs (a fetch, an install), read again until it ends.
  if (p && p.busy && Object.keys(p.busy).length) setTimeout(() => { if (S.view === 'settings' && S.setSection === 'plugins') loadPluginsView().then(render) }, 1000)
}
function pluginOp(op, body = {}) {
  send({ cmd: 'plugin_op', op, ...body })
  // The Inbox too: a plugin updated leaves it at once.
  setTimeout(() => { loadAttention(true); loadPluginsView().then(render) }, 300)
}
/** One of a plugin's own settings (its manifest's `settings`): a switch, or a box saved when it changes; a
 *  secret is never shown back, only whether it is set (Forget empties it). */
function plField(id, d) {
  const at = `data-pl-set="${esc(id)}" data-key="${esc(d.key)}"`
  const box = `id="pl-${esc(id)}-${esc(d.key)}" ${at} aria-label="${esc(d.label)}" spellcheck="false"`
  if (d.type === 'bool') {
    const on = d.value === true || d.value === 'true'
    return `<button class="sw${on ? ' on' : ''}" role="switch" aria-checked="${on}" aria-label="${esc(d.label)}" ${at} data-val="${!on}"></button>`
  }
  if (d.type === 'secret') return `<input class="set-in" type="password" ${box} placeholder="${d.set ? '•••• set — type to replace' : 'not set'}" autocomplete="off">${d.set ? `<button class="btn ghost" ${at} data-forget>Forget</button>` : ''}`
  return `<input class="set-in" ${box} value="${esc(d.value == null ? '' : String(d.value))}" placeholder="${d.type === 'folder' ? '/path/to/folder' : ''}">`
}
/** A plugin's setting being typed (not saved yet): renders wait until the box is left, so the text isn't lost. */
const plTyping = () => { const a = document.activeElement; return !!(a && a.matches && a.matches('input[data-pl-set]') && a.value !== a.defaultValue) }
document.addEventListener('change', (e) => {
  const t = e.target
  if (!t.matches || !t.matches('input[data-pl-set]')) return
  pluginOp('set', { id: t.dataset.plSet, key: t.dataset.key, value: t.value })
  if (t.type === 'password') t.value = ''
  t.defaultValue = t.value
})
document.addEventListener('focusout', (e) => {
  if (e.target.matches && e.target.matches('input[data-pl-set]') && S.renderOnBlur) { S.renderOnBlur = false; setTimeout(render) }
})
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('button[data-pl-set]')
  if (!b) return
  if ('forget' in b.dataset) pluginOp('set', { id: b.dataset.plSet, key: b.dataset.key, value: '' })
  else pluginOp('set', { id: b.dataset.plSet, key: b.dataset.key, value: b.dataset.val === 'true' })
})
/** Settings → Plugins (designed on the Workbench canvas): Installed — a row each, the chosen one on the right;
 *  Discover — categories, featured, cards, the install sheet over the page; Marketplaces — where plugins come from. */
const PL_COLORS = ['#9b78ff', '#4fb3d9', '#3a9cf0', '#e0a96d', '#3ccf91', '#e07fa3', '#d9a84f', '#7fd0c0']
const plColor = (id) => PL_COLORS[[...String(id)].reduce((h, ch) => (h * 31 + ch.charCodeAt(0)) >>> 0, 7) % PL_COLORS.length]
/** A plugin's tile: the icon of its own Settings section when it has one, else its initial, in its colour. */
function plTile(x, big = false) {
  const sec = Plugins.sections.find((s) => s.plugin === x.id)
  const inner = sec && sec.icon ? sec.icon : esc((x.name || x.id || '?').trim().charAt(0).toUpperCase())
  return `<span class="pl-tile${big ? ' big' : ''}" style="--c:${plColor(x.id)}">${inner}</span>`
}
const plMatch = (x) => { const q = (S.plQuery || '').trim().toLowerCase(); return !q || `${x.name} ${x.summary || ''} ${x.id} ${(x.categories || []).join(' ')}`.toLowerCase().includes(q) }
function pluginsSettings() {
  const p = S.plugins
  if (!p) { if (!S.pluginsLoading) { S.pluginsLoading = true; loadPluginsView().then(() => { S.pluginsLoading = false; render() }) } return '<h2>Plugins</h2><div class="skeleton" style="padding:20px"><i></i><i></i><i></i></div>' }
  // Shown again after a while: asked again (the app reads stale marketplaces then), the page keeps what it has meanwhile.
  if (!S.pluginsLoading && Date.now() - (S.pluginsAt || 0) > 60000) { S.pluginsLoading = true; loadPluginsView().then(() => { S.pluginsLoading = false; render() }) }
  if (p.error) return `<h2>Plugins</h2><p class="error">${esc(p.error)}</p>`
  const tab = S.plTab || ((p.installed || []).length ? 'installed' : 'discover')
  const tabs = [['installed', 'Installed', p.installed.length || ''], ['discover', 'Discover', ''], ['markets', 'Marketplaces', p.markets.length]]
  const head = `<div class="pl-head"><h2>Plugins</h2>${tab === 'markets' ? '' : `<input class="search pl-q" id="pl-q" placeholder="Search plugins" value="${esc(S.plQuery || '')}" aria-label="Search plugins">`}</div>`
  const bar = `<div class="tabs pl-tabs" role="tablist">${tabs.map(([id, l, n]) => `<button class="tab" role="tab" data-pl-tab="${id}" aria-selected="${tab === id}">${esc(l)}${n !== '' ? `<span class="n">${esc(String(n))}</span>` : ''}</button>`).join('')}</div>`
  const body = tab === 'installed' ? plInstalled(p) : tab === 'discover' ? plDiscover(p) : plMarkets(p)
  const sheet = S.plSheet ? `<div class="pl-scrim" data-pl-sheet-cancel>${plSheet(S.plSheet)}</div>` : ''
  return `<div class="pl-page">${head}${bar}${body}${sheet}</div>`
}
function plInstalled(p) {
  if (!p.installed.length) return '<p class="set-sub">No plugins yet — Discover lists what your marketplaces offer.</p>'
  const list = p.installed.filter(plMatch).sort((a, b) => a.name.localeCompare(b.name))
  const sel = p.installed.find((x) => x.id === S.plSel) || list[0] || p.installed[0]
  const row = (x) => {
    const pill = x.update && !x.update.blocked ? `<span class="pl-pill up">Update to ${esc(x.update.version)}</span>`
      : Plugins.errors[x.id] || p.errors && p.errors[`plugin:${x.id}`] ? '<span class="pl-pill bad">Has a problem</span>' : '<span></span>'
    return `<div class="pl-row${x.id === sel.id ? ' on' : ''}${x.enabled ? '' : ' off'}" data-pl-sel="${esc(x.id)}" role="group" aria-label="${esc(x.name)}">${plTile(x)}<div class="pl-txt"><div class="pl-n">${esc(x.name)} <span class="pl-v">${esc(x.version)}</span></div><div class="pl-s">${esc(x.summary || '')}</div></div>${pill}<button class="sw${x.enabled ? ' on' : ''}" role="switch" aria-checked="${!!x.enabled}" aria-label="${esc(x.name)} on" data-pl-enable="${esc(x.id)}" data-on="${!x.enabled}"></button></div>`
  }
  const rows = list.length ? list.map(row).join('') : '<p class="set-sub" style="padding:12px 14px">Nothing matches.</p>'
  const note = '<p class="set-sub">Where Echo sends you things — Teams, mail and the ways plugins add — is in <a href="#settings/deliver">Sends to you</a>.</p>'
  return `<div class="pl-split"><div><section class="pl-list">${rows}</section>${note}</div>${plDetail(p, sel)}</div>`
}
/** The chosen installed plugin: where it comes from, what it may do, its settings, update and remove. */
function plDetail(p, x) {
  const busy = plBusy(p, `plugin:${x.id}`)
  const sec = Plugins.sections.find((s) => s.plugin === x.id)
  const upd = x.update ? (x.update.blocked ? `<div class="set-sub">${esc(x.update.version)}: ${esc(x.update.blocked)}</div>` : `<button class="btn" data-pl-update="${esc(x.id)}"${busy ? ' disabled' : ''}>${busy ? esc(busy) : `Update to ${esc(x.update.version)}`}</button>`) : ''
  const open = sec && x.enabled ? `<button class="btn primary" data-pl-open="${esc(sec.id)}">Open its settings</button>` : ''
  const rm = S.plRemove === x.id ? `<div class="confirm"><span>Remove ${esc(x.name)}? Its settings and data can stay for when you install it again.</span>
    <button class="btn" data-pl-remove-cancel>Cancel</button><button class="btn" data-pl-remove="${esc(x.id)}" data-keep="1">Remove, keep data</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-pl-remove="${esc(x.id)}" data-keep="0">Remove with data</button></div>` : ''
  // Its own settings, stacked (the column is narrow): the name and its help above, the field the full width below.
  const fields = (x.settings || []).map((d) => `<div class="pl-field"><b>${esc(d.label)}</b>${d.help ? `<span class="set-sub">${esc(d.help)}</span>` : ''}<div class="pl-field-ctl">${plField(x.id, d)}</div></div>`)
  let own = ''
  const card = Plugins.settings[x.id]
  if (card && x.enabled && !sec) { try { own = card.render() } catch (e) { own = `<p class="error">${esc(String(e))}</p>` } }
  const broke = (Plugins.errors[x.id] ? `<p class="error">Its window part didn't load: ${esc(Plugins.errors[x.id])}</p>` : '')
    + (x.work_error && x.enabled ? `<p class="set-sub">Its items in ${esc(x.work_error.place)} weren't updated the last time: ${esc(x.work_error.error)}</p>` : '')
  return `<section class="pl-detail" aria-label="${esc(x.name)}">
    <div class="pl-dhead">${plTile(x, true)}<div style="flex:1;min-width:0"><div class="pl-dn">${esc(x.name)}</div><div class="set-sub">${esc(x.version)} · from <b>${esc(x.market)}</b>${x.market_gone ? ' (removed — no updates)' : ''} · commit <code>${esc((x.commit || '').slice(0, 10))}</code></div></div>
      <button class="sw${x.enabled ? ' on' : ''}" role="switch" aria-checked="${!!x.enabled}" aria-label="${esc(x.name)} on" data-pl-enable="${esc(x.id)}" data-on="${!x.enabled}"></button></div>
    ${x.summary ? `<p class="pl-sum">${esc(x.summary)}</p>` : ''}
    <div class="pl-acts">${open}${upd}<span class="grow"></span><button class="btn ghost danger" data-pl-ask-remove="${esc(x.id)}">Remove…</button></div>
    ${rm}${plErr(p, `plugin:${x.id}`)}${broke}
    ${fields.length ? `<div class="pl-h4">Its settings</div><div class="pl-fields">${fields.join('')}</div>` : ''}${own}
    <div class="pl-h4">What it can do</div>${plUses(x.uses)}
    <p class="set-sub">You agreed to these when you installed it. An update that asks for more asks you again.</p>
  </section>`
}
function plDiscover(p) {
  const all = (p.discover || []).filter(plMatch)
  const cats = [...new Set((p.discover || []).flatMap((x) => x.categories || []))].sort()
  const cat = cats.includes(S.plCat) ? S.plCat : ''
  const list = all.filter((x) => !cat || (x.categories || []).includes(cat))
    .sort((a, b) => (b.featured - a.featured) || (b.built_in - a.built_in) || a.name.localeCompare(b.name))
  const chips = cats.length ? `<div class="pl-cats" role="group" aria-label="Categories">${['', ...cats].map((k) => `<button class="pl-cat${k === cat ? ' on' : ''}" data-pl-cat="${esc(k)}">${esc(k ? k[0].toUpperCase() + k.slice(1) : 'All')}</button>`).join('')}</div>` : ''
  const reading = plBusy(p, 'markets') ? '<p class="set-sub">Reading marketplaces…</p>' : ''
  if (!list.length) return chips + reading + `<p class="set-sub">${S.plQuery || cat ? 'Nothing matches.' : 'Your marketplaces offer nothing yet' + ((p.markets || []).some((m) => m.error) ? ' — one can\'t be read (Marketplaces).' : '.')}</p>`
  const card = (x) => {
    const busy = plBusy(p, `plugin:${x.id}`)
    const act = x.installed === true ? '<span class="set-ok">Installed ✓</span>'
      : x.installed === false ? '<span class="set-sub">Installed from another marketplace</span>'
      : x.why_not ? `<span class="tag" title="${esc(x.why_not)}">${!x.version ? "Can't be read" : 'Needs a newer Echo'}</span>`
      : `<button class="btn primary" data-pl-review="${esc(x.id)}" data-market="${esc(x.market)}"${busy ? ' disabled' : ''}>${busy ? esc(busy) : 'Install'}</button>`
    const cats2 = (x.categories || []).map((k) => k[0].toUpperCase() + k.slice(1)).join(' · ')
    return `<div class="pl-card" role="group" aria-label="${esc(x.name)}">${''}<div class="pl-ctop">${plTile(x)}<div style="flex:1;min-width:0"><div class="pl-n">${esc(x.name)}</div><div class="pl-cat2">${esc(cats2)}</div></div><span class="tag${x.built_in ? ' rec' : ''}">${esc(x.market)}</span></div>
      <div class="pl-cs">${esc(x.summary || '')}</div><div class="pl-cact">${act}${x.version ? `<span class="pl-v">${esc(x.version)}</span>` : ''}</div>${plErr(p, `plugin:${x.id}`)}</div>`
  }
  const featured = !S.plQuery && !cat ? list.filter((x) => x.featured) : []
  const rest = list.filter((x) => !featured.includes(x))
  return chips + reading + (featured.length ? `<div class="pl-h4">Featured</div><div class="pl-grid two">${featured.map(card).join('')}</div>` : '') +
    (rest.length ? `${featured.length ? '<div class="pl-h4">All plugins</div>' : ''}<div class="pl-grid">${rest.map(card).join('')}</div>` : '')
}
/** The permission sheet, over the page: what the plugin will be able to do, in Echo's words, before anything
 *  is installed — and who listed it when Echo didn't. */
function plSheet(sh) {
  const r = sh.review
  const box = (inner, name = '') => `<div class="pl-sheet" role="dialog" aria-modal="true"${name ? ` aria-label="${esc(name)}"` : ''}>${inner}</div>`
  if (!r) return box('<span class="set-sub">Reading what it uses…</span>')
  if (r.error) return box(`<span class="error">${esc(r.error)}</span><div class="pl-sheet-btns"><button class="btn" data-pl-sheet-cancel>Close</button></div>`)
  const lines = sh.update ? r.new_uses : r.uses
  const outside = S.plugins && !(S.plugins.markets || []).some((m) => m.name === r.market && m.built_in)
  const who = outside ? `<div class="pl-warn"><b>!</b><span>Listed by ${esc(r.market)} — Echo hasn’t reviewed it. Install it only if you trust who made it.</span></div>` : ''
  const go = sh.update ? `Update to ${esc(r.version)}` : 'Install'
  return box(`<div class="pl-dhead">${plTile({ id: r.id, name: r.name })}<div style="flex:1"><div class="pl-dn">${sh.update ? `${esc(r.name)} ${esc(r.version)} asks for more` : `Install ${esc(r.name)} ${esc(r.version)}?`}</div>
      <div class="set-sub">from <b>${esc(r.market)}</b> · commit <code>${esc((r.commit || '').slice(0, 10))}</code></div></div></div>
    ${who}<div class="pl-h4">It will be able to</div>${plUses(lines)}
    ${r.blocked ? `<div class="error">${esc(r.blocked)}</div>` : ''}
    <div class="pl-sheet-btns"><button class="btn${outside ? ' primary' : ''}" data-pl-sheet-cancel${outside ? ' autofocus' : ''}>Cancel</button>${r.blocked ? '' : `<button class="btn${outside ? '' : ' primary'}" data-pl-sheet-go>${go}</button>`}</div>`, sh.update ? `${r.name} ${r.version} asks for more` : `Install ${r.name} ${r.version}?`)
}
function plMarkets(p) {
  const rows = p.markets.map((m) => {
    const busy = plBusy(p, `market:${m.name}`)
    const state = busy ? esc(busy) : m.error ? `<span class="error">Can’t be read: ${esc(m.error)}</span>` : m.fetched_at ? `${m.count} plugin${m.count === 1 ? '' : 's'} · read ${esc(relDay(m.fetched_at))}` : 'Not read yet'
    const tags = `${m.built_in ? ' <span class="tag rec">built in</span>' : ''}${m.hidden ? ' <span class="tag">hidden</span>' : ''}`
    const menu = S.plMenu === m.name ? `<div class="pl-pop" role="menu"><button role="menuitem" data-pl-mhide="${esc(m.name)}" data-hidden="${!m.hidden}">${m.hidden ? 'Show its plugins in Discover' : 'Hide its plugins from Discover'}</button>${m.built_in ? '' : `<button role="menuitem" class="danger" data-pl-mremove="${esc(m.name)}">Remove — installed plugins stay, without updates</button>`}</div>` : ''
    return `<div class="pl-mrow" role="group" aria-label="${esc(m.name)}">${plTile({ id: m.name, name: m.name })}<div class="pl-txt"><div class="pl-n">${esc(m.name)}${m.title && m.title !== m.name ? ` <span class="set-sub">(${esc(m.title)})</span>` : ''}${tags}</div><div class="pl-s"><code>${esc(m.source)}</code></div><div class="pl-s">${state}</div></div>
      <button class="btn" data-pl-mrefresh="${esc(m.name)}"${busy ? ' disabled' : ''}>Read again</button><span class="pl-menu"><button class="btn pl-ib" data-pl-mmenu="${esc(m.name)}" aria-label="More">⋯</button>${menu}</span></div>`
  })
  const add = `<section class="pl-add"><b>Add a marketplace</b>
    <div class="pl-addrow"><input class="set-in" id="pl-msrc" placeholder="owner/repo, a git address, or a folder" aria-label="Marketplace" spellcheck="false"><input class="set-in" id="pl-mname" placeholder="Name (optional)" aria-label="Name" spellcheck="false"><button class="btn primary" data-pl-madd>Add</button></div>
    <div class="set-sub">It needs <code>echo-marketplace.json</code> at its root. Private repositories use your own git sign-in. Plugins from it say who listed them when you install.</div></section>`
  return `<p class="set-sub">Where plugins come from. Echo’s own is built in; add your team’s or company’s catalog — a GitHub repository, any git address, or a folder.</p>
    <section class="pl-list pl-mlist">${rows.join('')}</section>${Object.keys(p.errors || {}).filter((k) => k.startsWith('market:')).map((k) => plErr(p, k)).join('')}${add}`
}
// The page's own controls: tabs, install / update (through the sheet), switch, remove, marketplaces, search.
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-pl-tab],[data-pl-enable],[data-pl-update],[data-pl-review],[data-pl-sheet-go],[data-pl-sheet-cancel],[data-pl-ask-remove],[data-pl-remove],[data-pl-remove-cancel],[data-pl-mrefresh],[data-pl-mhide],[data-pl-mremove],[data-pl-madd],[data-pl-sel],[data-pl-cat],[data-pl-mmenu],[data-pl-open]')
  if (!t) { if (S.plMenu) { S.plMenu = null; render() } return }
  const d = t.dataset
  // The sheet's backdrop closes it; a click inside the sheet doesn't.
  if ('plSheetCancel' in d && t.classList.contains('pl-scrim') && e.target.closest('.pl-sheet')) return
  if (d.plTab) { S.plTab = d.plTab; S.plSheet = null; S.plMenu = null; return render() }
  if (d.plEnable) { e.stopPropagation(); return pluginOp('enable', { id: d.plEnable, on: d.on === 'true' }) }
  if (d.plSel) { S.plSel = d.plSel; S.plRemove = null; return render() }
  if (d.plCat != null && t.classList.contains('pl-cat')) { S.plCat = d.plCat; return render() }
  if (d.plOpen) return go(`settings/${d.plOpen}`)
  if (d.plMmenu) { S.plMenu = S.plMenu === d.plMmenu ? null : d.plMmenu; return render() }
  if (d.plMhide || d.plMremove) S.plMenu = null
  if (d.plReview || d.plUpdate) {
    const id = d.plReview || d.plUpdate, market = d.market || ''
    S.plSheet = d.plUpdate ? { update: id, review: (S.plugins.installed.find((x) => x.id === id) || {}).update } : { install: id, market, review: null }
    render()
    if (d.plReview) get(`api/plugins/review?id=${encodeURIComponent(id)}&market=${encodeURIComponent(market)}`).then((r) => { if (S.plSheet && S.plSheet.install === id) { S.plSheet.review = r; render() } })
    return
  }
  if ('plSheetCancel' in d) { S.plSheet = null; return render() }
  if ('plSheetGo' in d) {
    const sh = S.plSheet
    S.plSheet = null
    if (sh && sh.review) pluginOp(sh.update ? 'update' : 'install', { id: sh.review.id, market: sh.review.market, agreed: sh.update ? sh.review.new_uses : sh.review.uses })
    return render()
  }
  if (d.plAskRemove) { S.plRemove = d.plAskRemove; return render() }
  if ('plRemoveCancel' in d) { S.plRemove = null; return render() }
  if (d.plRemove) { S.plRemove = null; return pluginOp('remove', { id: d.plRemove, keep_data: d.keep === '1' }) }
  if (d.plMrefresh) return pluginOp('market_refresh', { name: d.plMrefresh })
  if (d.plMhide) return pluginOp('market_hide', { name: d.plMhide, hidden: d.hidden === 'true' })
  if (d.plMremove) return pluginOp('market_remove', { name: d.plMremove })
  if ('plMadd' in d) {
    const source = ($('#pl-msrc') || {}).value || '', name = ($('#pl-mname') || {}).value || ''
    if (source.trim()) pluginOp('market_add', { source: source.trim(), name: name.trim() })
  }
})
document.addEventListener('input', (e) => {
  if (e.target.id !== 'pl-q') return
  S.plQuery = e.target.value
  const pos = e.target.selectionStart
  render()
  const q = $('#pl-q')
  if (q) { q.focus(); q.setSelectionRange(pos, pos) }
})
const plUses = (uses) => (uses || []).length ? `<ul class="pl-uses">${uses.map((u) => `<li>${esc(u)}</li>`).join('')}</ul>` : '<div class="set-sub">Uses nothing of Echo\'s.</div>'
const plBusy = (p, key) => p.busy && p.busy[key]
const plErr = (p, key) => p.errors && p.errors[key] ? `<div class="error">${esc(p.errors[key])}</div>` : ''
async function loadDeliver() {
  try { S.deliver = await get('api/deliver') } catch { /* app restarting */ }
}
function deliverOp(op, body = {}) {
  send({ cmd: 'deliver_op', op, ...body })
  setTimeout(() => loadDeliver().then(render), 500)
}
/** Where Echo sends things to you: your Teams chat with yourself, your mailbox — and the ways plugins add
 *  (Telegram…), each set up in its own plugin's section. */
function deliverCard(d) {
  const test = (k) => `<button class="btn ghost" data-dl-test="${esc(k)}">Send a test</button>`
  const ms = d.account ? `As ${esc(d.account)}.` : 'Sign in to Microsoft 365 first (Settings → Calendar &amp; chat).'
  const own = (d.provided || []).map((x) => {
    const sec = Plugins.sections.find((s) => s.plugin === x.plugin)
    return setRow(esc(x.label), x.ready ? `Ready — from the ${esc(x.plugin)} plugin.` : `${esc(x.why || 'Not set up yet')} — in its plugin’s section.`,
      (x.ready ? test(x.kind) : '') + (sec ? `<button class="btn${x.ready ? ' ghost' : ' primary'}" data-dl-go="${esc(sec.id)}">${x.ready ? 'Settings' : 'Set up…'}</button>` : ''))
  })
  return setCard('', [
    setRow('Teams', `Your chat with yourself. ${ms}`, d.teams ? test('teams') : '', '', 'Teams'),
    setRow('Mail', `Your own mailbox — or the addresses a plugin was given. ${ms}`, d.mail ? test('mail') : '', '', 'Mail'),
    ...own,
  ])
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-dl-test],[data-dl-go]')
  if (!t) return
  if (t.dataset.dlGo) return go(`settings/${t.dataset.dlGo}`)
  if (t.dataset.dlTest) return deliverOp('test', { kind: t.dataset.dlTest })
})

