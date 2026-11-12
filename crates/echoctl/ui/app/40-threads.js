// ── Projects (`projects.rs`, docs/projects-design.md) ─────────────────────────────────────────
/** Symbols a project can wear instead of its identicon (24-unit strokes, drawn in its colour). */
const PJ_SYMBOLS = {
  rocket: '<path d="M12 3c3 2 5 5.5 5 9.5l-2.5 3h-5L7 12.5C7 8.5 9 5 12 3z"/><circle cx="12" cy="10" r="1.6"/><path d="M9.5 15.5 7 19l3-.8M14.5 15.5 17 19l-3-.8"/>',
  database: '<ellipse cx="12" cy="6" rx="7" ry="2.5"/><path d="M5 6v12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5V6M5 12c0 1.4 3.1 2.5 7 2.5s7-1.1 7-2.5"/>',
  shield: '<path d="M12 3l7 3v5c0 4.5-3 8-7 10-4-2-7-5.5-7-10V6z"/><path d="M9 12l2 2 4-4"/>',
  people: '<circle cx="9" cy="9" r="3"/><path d="M3.5 19c.6-3 2.8-5 5.5-5s4.9 2 5.5 5"/><circle cx="17" cy="8" r="2.4"/><path d="M15.5 13.3c2.4.2 4.3 2 4.8 4.7"/>',
  chart: '<path d="M4 20V4M4 20h16"/><path d="M8 16v-4M12 16V8M16 16v-6"/>',
  cloud: '<path d="M7 18h10a4 4 0 0 0 .5-8 5.5 5.5 0 0 0-10.6 1.5A3.3 3.3 0 0 0 7 18z"/>',
  code: '<path d="M9 8l-4 4 4 4M15 8l4 4-4 4M13 6l-2 12"/>',
  briefcase: '<rect x="3.5" y="7.5" width="17" height="12" rx="2"/><path d="M9 7.5V6a1.5 1.5 0 0 1 1.5-1.5h3A1.5 1.5 0 0 1 15 6v1.5M3.5 12.5h17"/>',
  flask: '<path d="M10 3.5h4M10.5 3.5v6L5 18.5a1.3 1.3 0 0 0 1.1 2h11.8a1.3 1.3 0 0 0 1.1-2l-5.5-9v-6"/><path d="M7.5 15h9"/>',
  globe: '<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.4 3.6 5.2 3.6 8.5s-1.2 6.1-3.6 8.5c-2.4-2.4-3.6-5.2-3.6-8.5S9.6 5.9 12 3.5z"/>',
  cart: '<path d="M3.5 4.5h2.5l2 11h10l2-8H7"/><circle cx="9.5" cy="19" r="1.3"/><circle cx="16.5" cy="19" r="1.3"/>',
  book: '<path d="M5 4.5h10.5a2.5 2.5 0 0 1 2.5 2.5v13H7.5A2.5 2.5 0 0 1 5 17.5z"/><path d="M5 17.5A2.5 2.5 0 0 1 7.5 15H18"/>',
}
const PJ_COLORS = ['#9b78ff', '#5aa9ff', '#3ccf91', '#f0a24a', '#e8603c', '#d36bd1', '#4cc3c8', '#c9b458']
/** A project's tile: its logo, else its symbol, else its identicon — `size` px square. */
function projIcon(p, size = 18) {
  const color = /^#[0-9a-f]{6}$/i.test(p.color || '') ? p.color : PJ_COLORS[0]
  const t = `class="pj-ic" style="--pj:${color};width:${size}px;height:${size}px" title="${esc(p.name || '')}"`
  if (p.has_logo) return `<img ${t} src="api/project/${encodeURIComponent(p.id)}/logo?v=${esc(p.logo_v || 0)}" alt="">`
  if (p.icon === 'client' && p.client && p.client_logo_v) return `<img ${t} src="api/client/${encodeURIComponent(p.client)}/logo?v=${esc(p.client_logo_v)}" alt="">`
  const sym = PJ_SYMBOLS[p.icon]
  if (sym) return `<span ${t} aria-hidden="true"><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">${sym}</svg></span>`
  const cells = ProjectArt.cells(p.id || p.name || '')
  const rects = cells.map((on, i) => on ? `<rect x="${i % 5}" y="${Math.floor(i / 5)}" width="1" height="1"/>` : '').join('')
  return `<span ${t} aria-hidden="true"><svg viewBox="-0.75 -0.75 6.5 6.5" fill="currentColor" shape-rendering="crispEdges">${rects}</svg></span>`
}
/** A client's logo (nothing when it has none). */
const clientIcon = (c, size = 16) => c && c.logo_v ? `<img class="pj-ic" style="width:${size}px;height:${size}px" src="api/client/${encodeURIComponent(c.id)}/logo?v=${esc(c.logo_v)}" alt="" title="${esc(c.name)}">` : ''
/** A logo file the user chose or dropped → the app (as base64), after a size check here. */
function sendLogo(file, op, id) {
  if (!file) return
  if (!/^image\/(png|jpeg|webp)$/.test(file.type)) return window.echo.notice('A logo is a PNG, JPEG or WebP image.')
  if (file.size > 512 * 1024) return window.echo.notice('A logo is at most 512 KB.')
  const r = new FileReader()
  r.onload = () => send({ cmd: 'project', op, id, data: String(r.result).replace(/^data:[^,]*,/, '') })
  r.readAsDataURL(file)
}
const logoPicker = (op, id, label) => `<label class="btn pj-file">${esc(label)}<input type="file" accept="image/png,image/jpeg,image/webp" data-logo-op="${op}" data-id="${esc(id)}" aria-label="${esc(label)}"></label>`
const stageTag = (id, long = false) => { const st = ProjectArt.stage(id); return `<span class="pj-stage ${esc(st.gid)}">${esc(long && st.group ? `${st.group} · ${st.label}` : st.label)}</span>` }
/** A link to a thread's Brief (a hash, so the router opens it). */
const pjA = (l, text = l.name) => `<a class="link-btn" href="#project/${encodeURIComponent(l.id)}/brief">${esc(text)}</a>`
/** The header's thread links (#95): "Part of <thread> →" and "Related: <threads>"; nothing without links. */
function pjLinksLine(p) {
  const ls = p.links || [], parent = ls.find((l) => l.kind === 'part_of'), rel = ls.filter((l) => l.kind === 'related')
  const bits = [parent ? `<span>Part of ${pjA(parent, `${parent.name} →`)}</span>` : '', rel.length ? `<span>Related: ${rel.map((l) => pjA(l)).join(', ')}</span>` : ''].filter(Boolean)
  return bits.length ? `<div class="facts pj-links">${bits.join('')}</div>` : ''
}
const LINK_KIND = { part_of: 'Is part of', part: 'Part of this', related: 'Related' }
const LINK_ITEM = { task: 'Task', risk: 'Risk', question: 'Question', decision: 'Decision' }
/** Brief → Linked threads: each linked thread with its stage, its open items and its latest decisions. */
function pjLinked(p) {
  const ls = p.links || []
  if (!ls.length) return ''
  const item = (x) => `<li><b>${LINK_ITEM[x.kind] || esc(x.kind)}</b> <a href="${esc(x.link)}">${esc(x.title)}</a>${x.owner ? ` · ${esc(x.owner)}` : ''}${x.late ? ' · <span class="warn">late</span>' : ''}</li>`
  const rows = ls.map((l) => {
    const w = l.work || {}, items = [...(w.open || []), ...(w.decisions || [])]
    return `<div class="set-row pj-link" role="listitem"><div style="min-width:0"><div class="pj-name"><span class="set-sub">${LINK_KIND[l.kind] || ''}</span>${pjA(l)}${stageTag(l.stage)}<span class="set-sub">${w.open_count || 0} open</span></div>${items.length ? `<ul class="pj-link-items">${items.map(item).join('')}</ul>` : ''}</div></div>`
  }).join('')
  return `<section class="set-sec"><h3>Linked threads</h3><div class="set-card" role="list" aria-label="Linked threads">${rows}</div></section>`
}
const projClient = (id) => ((S.pj.data && S.pj.data.clients) || []).find((c) => c.id === id)

/** Draws the Threads view in the primary area (`renderNow`). */
function drawProjects(primary) {
  patch(primary, `<main class="main">${projectsView()}</main>`)
  drawOrgLinks()
  // A plugin's Profile row draws into its box once it is in (`thread.profile`'s after).
  const p = S.pj.detail
  if (!p || p.error || S.pj.tab !== 'profile') return
  for (const x of Plugins.slots['thread.profile'] || []) {
    const box = x.after && primary.querySelector(`[data-thread-slot="${CSS.escape(`${x.plugin}:${x.id}`)}"]`)
    try { if (box) x.after(box, slotThread(p)) } catch (e) { console.warn(`thread.profile: ${x.plugin}: ${(e && e.message) || e}`) }
  }
}
function projectsView() {
  const data = S.pj.data
  if (!data) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(6)}</div>`
  return `<div class="projects"><aside class="pj-side">${projectList(data)}</aside><div class="pj-main">${projectMain(data)}</div></div>`
}

/** The Threads list's clients folded by the user (an accordion each; remembered in the webview). */
const PJ_FOLD_KEY = 'echo.threads.folded'
const pjFolded = (() => { try { return new Set(JSON.parse(localStorage.getItem(PJ_FOLD_KEY) || '[]')) } catch { return new Set() } })()
function pjFold(id) {
  if (pjFolded.has(id)) pjFolded.delete(id); else pjFolded.add(id)
  try { localStorage.setItem(PJ_FOLD_KEY, JSON.stringify([...pjFolded])) } catch { /* storage off */ }
}
function projectList(data) {
  const ps = data.projects || []
  const clients = data.clients || []
  const row = (p) => {
    const last = p.last_call ? dayLabel(new Date(p.last_call)) : ''
    return `<button class="pj-row${p.id === S.pj.sel ? ' on' : ''}" data-pj="open" data-id="${esc(p.id)}">${projIcon(p, 20)}<span class="pj-row-t"><b>${esc(p.name)}</b><span class="pj-row-m">${p.call_count} call${p.call_count === 1 ? '' : 's'}${last ? ` · ${esc(last)}` : ''}</span></span>${p.group !== 'closed' ? stageTag(p.stage) : ''}</button>`
  }
  const open = ps.filter((p) => p.group !== 'closed'), closed = ps.filter((p) => p.group === 'closed')
  const groups = []
  // Each client an accordion: the chevron folds it (the open thread stays in view), the name opens its page.
  const fold = (id, xs, name, label) => {
    const folded = pjFolded.has(id), shown = folded ? xs.filter((p) => p.id === S.pj.sel) : xs
    // Nothing to fold: a spacer keeps the names in line.
    const chev = !xs.length ? '<span class="pj-chev" aria-hidden="true"></span>' : `<button class="pj-chev" data-pj="fold" data-id="${esc(id)}" aria-expanded="${!folded}" title="${folded ? 'Show' : 'Hide'} its threads" aria-label="${folded ? 'Show' : 'Hide'} its threads">${ICON(folded ? '<path d="M9 6l6 6-6 6"/>' : '<path d="M6 9l6 6 6-6"/>')}</button>`
    return `<div class="pj-ghead${folded ? ' folded' : ''}" role="group" aria-label="${esc(label)}">${chev}${name}${folded && xs.length ? `<span class="pj-gn">${xs.length}</span>` : ''}</div>${shown.map(row).join('')}`
  }
  for (const c of [...clients.filter((c) => !c.internal), ...clients.filter((c) => c.internal)]) {
    const xs = open.filter((p) => p.client === c.id)
    groups.push(fold(c.id, xs, `<button class="pj-group pj-client${S.pj.client === c.id ? ' on' : ''}" data-pj="client" data-id="${esc(c.id)}" title="${esc(c.name)}: its details">${clientIcon(c, 14)}<span class="pj-cn">${esc(c.internal ? `${c.name} · your company` : c.name)}</span></button>`, c.internal ? `${c.name} · your company` : c.name))
  }
  // Folds of clients that are gone are forgotten.
  if (clients.length && !data.error) for (const k of [...pjFolded]) if (k !== '-' && !clients.some((c) => c.id === k)) pjFold(k)
  const none = open.filter((p) => !p.client || !clients.some((c) => c.id === p.client))
  if (none.length) groups.push(clients.length ? fold('-', none, '<div class="pj-group">No client</div>', 'No client') : `<div class="pj-group">Threads</div>${none.map(row).join('')}`)
  const closedRows = closed.length ? `<button class="pj-group pj-fold" data-pj="toggle-closed" aria-expanded="${S.pj.showClosed}">${CHEV(S.pj.showClosed)} Closed · ${closed.length}</button>${S.pj.showClosed ? closed.map(row).join('') : ''}` : ''
  const err = data.error ? `<p class="error" style="padding:10px">${esc(data.error)}</p>` : ''
  const fc = data.found_count || 0
  const found = `<button class="pj-row pj-found${S.pj.found ? ' on' : ''}${fc ? '' : ' quiet'}" data-pj="found"><span class="pj-spark" aria-hidden="true">✦</span><b class="grow">Found for you</b>${fc ? `<span class="pj-badge">${fc}</span>` : ''}</button>`
  return `<div class="pj-side-head"><h1>Threads</h1><button class="icon-btn" data-pj="new" title="New thread" aria-label="New thread">${ICON('<path d="M12 5v14M5 12h14"/>')}</button></div>${found}
    ${err}<div class="pj-rows">${groups.join('')}${closedRows}</div>`
}

function projectMain(data) {
  if (S.pj.found) return foundView(data)
  if (S.pj.client) return clientView(data)
  if (!S.pj.sel) {
    const n = (data.projects || []).length
    return `<div class="empty"><div>${mark()}<h3>${n ? 'Pick a thread' : 'No threads yet'}</h3><p>A thread gathers the calls of one piece of work — a client engagement, an initiative, hiring — by its meeting series, its people or its chat. Its glossary helps transcription, and the minutes see what was said in its other calls.</p><p>Name one in the box on the left to start.</p></div></div>`
  }
  const p = S.pj.detail
  if (!p) return `<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(8)}</div>`
  if (p.error) return `<div class="empty"><div><h3>This thread can't be opened</h3><p class="error">${esc(p.error)}</p></div></div>`
  const open = workOpenCount()
  const tabs = [['brief', 'Brief'], ['work', `Work${open ? `<span class="n">${open}</span>` : ''}`], ['calls', `Calls<span class="n">${p.call_count}</span>`], ['people', `People${peopleCount() ? `<span class="n">${peopleCount()}</span>` : ''}`], ['profile', 'Profile']]
  const tab = tabs.some(([t]) => t === S.pj.tab) ? S.pj.tab : 'brief'
  const client = p.client_name ? `<span class="tag pj-client-tag" title="Client">${clientIcon(projClient(p.client), 14)}${esc(p.client_name)}</span>` : ''
  const facts = [`${p.call_count} call${p.call_count === 1 ? '' : 's'}`, p.since ? `since ${esc(p.since)}` : '', p.last_call ? `last call ${esc(dayLabel(new Date(p.last_call)).toLowerCase())}` : ''].filter(Boolean).join(' · ')
  const menu = `<span class="pj-add-wrap pj-more"><button class="icon-btn" data-pj="menu" aria-haspopup="menu" aria-expanded="${S.pj.menu}" aria-label="Thread actions" title="Thread actions">${ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.pj.menu ? `<div class="menu" role="menu">${p.client ? `<button class="menu-item" role="menuitem" data-pj="client" data-id="${esc(p.client)}">Open ${esc(p.client_name || 'the client')}</button>` : ''}<button class="menu-item" role="menuitem" data-claude-about>Claude about this thread</button><button class="menu-item" role="menuitem" data-pj="sync">Sync now</button><div class="menu-sep" role="separator"></div><button class="menu-item danger" role="menuitem" data-pj="delete-ask">Delete…</button></div>` : ''}</span>`
  const confirmDel = S.pj.confirmDelete ? `<div class="notice"><span class="grow">Delete <b>${esc(p.name)}</b>? Its brief, profile and work go to the trash; its calls stay.</span><button class="btn ghost" data-pj="delete-cancel">Cancel</button><button class="btn danger" data-pj="delete">Move to trash</button></div>` : ''
  const head = `<header class="pj-head" role="group" aria-labelledby="pj-h"><div class="pj-title">${projIcon(p, 34)}<div style="min-width:0"><div class="pj-name"><h2 id="pj-h">${esc(p.name)}</h2>${client}${stageTag(p.stage, true)}</div><div class="facts"><span>${facts}</span></div>${pjLinksLine(p)}</div><span class="grow"></span>${menu}</div>${confirmDel}
    <div class="tabs" role="tablist">${tabs.map(([id, label]) => `<button class="tab" role="tab" data-pj="tab" data-tab-id="${id}" aria-selected="${tab === id}">${label}</button>`).join('')}</div></header>`
  const body = tab === 'calls' ? projectCalls(p) : tab === 'profile' ? projectProfile(p, data) : tab === 'work' ? projectWork(p) : tab === 'people' ? peopleView() : projectBrief(p)
  return `${head}<div class="pane pj-pane${tab === 'work' || tab === 'people' ? ' wide' : ''}">${body}</div>`
}

