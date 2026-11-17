// ── Knowledge's plugin tabs: a plugin's tabs (`knowledge.tab`) drawn after Recall's as a group under its name, its
// header strip (`knowledge.header`) over them, each tab's pane edge to edge with its own scroll, kept while the user
// moves between tabs; `#brain/<plugin>:<tab>/<rest>`, the rest handed to the tab as written.
// ── knowledge tabs: the plugins' tabs of Knowledge (no DOM: `knowledge-tabs.test.mjs`) — each plugin's tabs a group
// headed by its name, in the order the plugins added them, its tabs by `order` (else as added); a tab whose `visible()`
// is false (or throws) left out, a plugin with none shown has no group; a tab's `badge()` '' | n | '!' (one that throws:
// none); a second tab with the same plugin and id left out. Each problem goes to `log`.
const knBadge = (v) => v === '!' ? '!' : Number.isFinite(Number(v)) && v !== '' && v !== null && Number(v) >= 1 ? Math.floor(Number(v)) : ''
function knowledgeGroups(items, names, log) {
  const groups = [], seen = new Set()
  ;(items || []).forEach((x, at) => {
    const key = `${x.plugin}:${x.id}`
    if (seen.has(key)) { log(`${x.plugin}: ${x.label || x.id}: a second tab with the id “${x.id}” — left out`); return }
    seen.add(key)
    let shown = true
    if (x.visible) { try { shown = !!x.visible() } catch (e) { shown = false; log(`${x.plugin}: ${x.label || x.id}: visible(): ${(e && e.message) || e}`) } }
    if (!shown) return
    let badge = ''
    if (x.badge) { try { badge = knBadge(x.badge()) } catch (e) { log(`${x.plugin}: ${x.label || x.id}: badge(): ${(e && e.message) || e}`) } }
    let g = groups.find((y) => y.plugin === x.plugin)
    if (!g) groups.push(g = { plugin: x.plugin, name: String((names || {})[x.plugin] || x.plugin), tabs: [] })
    g.tabs.push({ key, plugin: x.plugin, label: String(x.label || x.id), icon: x.icon ? String(x.icon) : '', order: Number.isFinite(x.order) ? x.order : Infinity, at, badge, item: x })
  })
  for (const g of groups) g.tabs.sort((p, q) => (p.order - q.order) || (p.at - q.at))
  return groups
}
/** The activity bar's badge for Knowledge: '!' when any tab says '!', else the tabs' numbers summed ('' for none). */
function knowledgeBadge(groups) {
  const tabs = groups.flatMap((g) => g.tabs)
  if (tabs.some((t) => t.badge === '!')) return '!'
  const n = tabs.reduce((s, t) => s + (typeof t.badge === 'number' ? t.badge : 0), 0)
  return n || ''
}
/** `#brain/<tab>/<rest>` → {tab (decoded), rest (as written: its parts still encoded)}; another address → null. */
function knowledgeHash(hash) {
  const m = /^brain(?:\/([^/]*)(?:\/(.*))?)?$/.exec(String(hash || '').replace(/^#/, ''))
  if (!m) return null
  let tab = m[1] || ''
  try { tab = decodeURIComponent(tab) } catch { /* a broken %-sequence: as written */ }
  return { tab, rest: m[2] || '' }
}
/** The tab Knowledge shows for the one asked (`want`): it, when shown (a Recall tab only with Recall on); a plugin's
 *  hidden or unknown tab: that plugin's first; else Recall's first tab when it is on, or the first plugin tab; null: none. */
function knowledgeTab(want, recall, groups, recallTabs) {
  const tabs = groups.flatMap((g) => g.tabs)
  if (tabs.some((t) => t.key === want)) return want
  if (recall && recallTabs.includes(want)) return want
  const m = /^([a-z0-9-]+):/.exec(want || '')
  const own = m && groups.find((g) => g.plugin === m[1])
  if (own) return own.tabs[0].key
  if (recall) return recallTabs[0]
  return tabs.length ? tabs[0].key : null
}
/** The tab row: Recall's tabs (`[id, label, count]`), then each plugin's group under its name, the shown one selected. */
function knowledgeTabRow(recallTabs, groups, active) {
  const recall = recallTabs.map(([id, l, n]) => `<button class="tab" role="tab" data-br-tab="${esc(id)}" aria-selected="${active === id}">${esc(l)}${n ? `<span class="n">${esc(String(n))}</span>` : ''}</button>`).join('')
  const badge = (b) => b === '!' ? '<span class="n kn-alert" title="Needs you">!</span>' : b ? `<span class="n">${esc(String(b))}</span>` : ''
  const grp = groups.map((g) => `<span class="kn-grp" role="group" aria-label="${esc(g.name)}"><span class="kn-gname" aria-hidden="true">${esc(g.name)}</span>${g.tabs.map((t) => `<button class="tab kn-tab" role="tab" data-br-tab="${esc(t.key)}" aria-selected="${active === t.key}">${t.icon}${esc(t.label)}${badge(t.badge)}</button>`).join('')}</span>`).join('')
  return recall + grp
}
/** Each plugin's header strip (`knowledge.header`): the first it added; another one is left out (logged). */
function knowledgeHeaders(items, log) {
  const out = {}
  for (const x of items || []) {
    if (out[x.plugin]) { log(`${x.plugin}: a second knowledge.header — left out`); continue }
    out[x.plugin] = x
  }
  return out
}
// end of the knowledge tabs block
const knLog = (m) => console.error(m)
/** The plugins' tab groups now (their `visible()` and `badge()` asked again). */
const knowledgeGroupsNow = () => knowledgeGroups(Plugins.slots['knowledge.tab'], Plugins.names, knLog)
/** Knowledge is in the activity bar with Recall on or any plugin tab shown (F-KNOW-01). */
const knowledgeShown = () => brainOn() || knowledgeGroupsNow().length > 0
/** A Knowledge tab's address, where the user left it (a plugin tab's rest). */
const knowledgeGo = (tab) => { const r = (S.br.rests || {})[tab]; return `brain/${tab}${r ? '/' + r : ''}` }
/** The tab Knowledge shows now. */
const knowledgeTabNow = (groups = knowledgeGroupsNow()) => knowledgeTab(S.br.tab, brainOn(), groups, BRAIN_TABS.map(([id]) => id))
const KN_GEAR = ICON('<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>')
/** The gear at the end of Knowledge's tab row: Settings → Knowledge (Recall's card, then the plugins'). */
const knGear = () => `<button class="icon-btn kn-gear" data-kn-settings aria-label="Knowledge settings" title="Knowledge settings">${KN_GEAR}</button>`
/** A plugin tab shown: the head (tabs, search, gear), its plugin's header strip, its pane — each tab its own box, kept
 *  while the user moves between tabs (no redraw from nothing, its scroll where it was). */
function drawKnowledgePlugin(primary, groups, kt) {
  const rest = S.br.rest || ''
  patch(primary, '<main class="main plug-act"><div class="br kn"><header class="pj-head br-head kn-head"><div class="br-title"><h2>Knowledge</h2></div><div class="kn-row"><div class="tabs" role="tablist" data-kn-tabs></div><input class="search kn-q" id="kn-q" type="search" hidden><span class="kn-end"></span></div></header><div class="kn-strip" hidden></div><div class="kn-panes"></div></div></main>')
  const recall = brainOn() ? BRAIN_TABS.map(([id, l]) => [id, l, '']) : []
  patch(primary.querySelector('[data-kn-tabs]'), knowledgeTabRow(recall, groups, kt.key))
  patch(primary.querySelector('.kn-end'), knGear())
  // The section's search: the tab's own (`onSearch`), unless it says `search: false`; never Recall's.
  const q = primary.querySelector('#kn-q')
  q.hidden = !(kt.item.onSearch && kt.item.search !== false)
  if (q._for !== kt.key) { q._for = kt.key; q.value = (S.br.kq || {})[kt.key] || ''; q.placeholder = `Search ${kt.label}`; q.setAttribute('aria-label', `Search ${kt.label}`) }
  // Its plugin's header strip, shared by its tabs.
  const strip = primary.querySelector('.kn-strip'), h = knowledgeHeaders(Plugins.slots['knowledge.header'], knLog)[kt.plugin]
  let sh = ''
  if (h) { try { sh = String(h.header() || '') } catch (e) { sh = `<p class="error">${esc(String((e && e.message) || e))}</p>` } }
  if (strip.dataset.plugin !== kt.plugin) { strip.dataset.plugin = kt.plugin; strip._html = null }
  strip.hidden = !sh
  patch(strip, sh)
  try { if (sh && h.after) h.after(strip) } catch (e) { console.error(e) }
  // Its pane: a box per tab, the others kept hidden; a tab gone (its plugin off, hidden now) loses its box.
  const panes = primary.querySelector('.kn-panes'), live = new Set(groups.flatMap((g) => g.tabs.map((t) => t.key)))
  for (const b of [...panes.children]) if (!live.has(b.dataset.ktab)) b.remove()
  let box = [...panes.children].find((b) => b.dataset.ktab === kt.key)
  if (!box) {
    box = document.createElement('div')
    box.className = 'kn-pane'; box.dataset.ktab = kt.key; box.hidden = true
    box.addEventListener('scroll', () => { box._top = box.scrollTop; box._left = box.scrollLeft }, { passive: true })
    panes.appendChild(box)
  }
  for (const b of panes.children) if (b !== box) b.hidden = true
  const shown = box.hidden
  box.hidden = false
  let html = ''
  try { html = kt.item.render(rest) } catch (e) { html = `<p class="error">${esc(kt.label)}: ${esc(String((e && e.message) || e))}</p>` }
  patch(box, html == null ? '' : String(html))
  if (shown && box._top != null) { box.scrollTop = box._top; box.scrollLeft = box._left }
  try { if (kt.item.after) kt.item.after(box, rest) } catch (e) { console.error(e) }
}
document.addEventListener('input', (e) => {
  if (e.target.id !== 'kn-q' || S.view !== 'brain') return
  const kt = knowledgeGroupsNow().flatMap((g) => g.tabs).find((t) => t.key === e.target._for)
  if (!kt || !kt.item.onSearch) return
  S.br.kq = { ...(S.br.kq || {}), [kt.key]: e.target.value }
  try { kt.item.onSearch(e.target.value) } catch (err) { console.error(err) }
})
document.addEventListener('click', (e) => {
  if (!(e.target.closest && e.target.closest('[data-kn-settings]'))) return
  S.setSection = 'brain'
  go('settings/brain')
})
