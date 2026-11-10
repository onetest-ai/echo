// ── org chart layout (no DOM; org-chart.test.mjs runs app.js from here to the end mark) ──────────
/** Who goes where on the org chart (#46): each person's workstream — their saved one (the thread's
 *  `group`, else the directory's), else the one their role names ("<X> stream lead", a "<X>:" prefix
 *  two people share, a stream name in the role: "Design (CX)") —, whether they lead it (a "<X> stream
 *  lead" role, or a "lead" role in their saved stream), and whether they sit in the governance band:
 *  set explicitly, else high influence and interest for people with no workstream only — a stream's
 *  lead leads its column, not the band. `place(x)` = their place in this view. */
const OrgLayout = (() => {
  const reEsc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  const streamOfRole = (role) => { const m = String(role || '').match(/(?:^|[:;(,]\s*)([A-Za-z][\w &/+-]{0,40}?)\s+(?:work)?stream\b/i); return m ? m[1].trim() : null }
  const isLead = (role) => /\blead\b/i.test(String(role || ''))
  const saved = (x, place) => place(x).group || x.group || ''
  /** Workstream names the thread gives (saved groups, "<X> stream" roles, a shared "<X>:" prefix) and
   *  whom a role puts in one. */
  function roleStreams(people, place) {
    const names = new Set(people.map((x) => saved(x, place)).filter(Boolean))
    const prefix = new Map()
    for (const x of people) {
      const r = String(place(x).role || '')
      const s = streamOfRole(r)
      if (s) names.add(s)
      const m = r.match(/^\s*([A-Z][\w &/+-]{1,40}):/)
      if (m) prefix.set(m[1].trim(), (prefix.get(m[1].trim()) || 0) + 1)
    }
    for (const [n, c] of prefix) if (c >= 2) names.add(n)
    const out = new Map()
    for (const x of people) {
      const r = String(place(x).role || '')
      const own = streamOfRole(r)
      const hit = own ? [...names].find((n) => n.toLowerCase() === own.toLowerCase()) || own : [...names].find((n) => new RegExp(`(^|[^\\w])${reEsc(n)}([^\\w]|$)`, 'i').test(r))
      if (hit) out.set(x.key, hit)
    }
    return out
  }
  function assign(people, place) {
    const roles = roleStreams(people, place)
    const out = new Map()
    for (const x of people) {
      const p = place(x), own = saved(x, place), role = p.role || ''
      const stream = own || roles.get(x.key) || ''
      const leads = isLead(role) && stream && (own ? true : !!streamOfRole(role))
      const band = p.governance != null ? !!p.governance : !stream && (p.influence || 0) >= 3 && (p.interest || 0) >= 3
      out.set(x.key, { stream, fromRoles: !own && !!stream, band, lead: leads && !band })
    }
    return out
  }
  return { assign, roleStreams, streamOfRole }
})()
// end of the org chart layout block
const ocPlace = (x) => placeOf(x) || {}
/** The chart's layout of the people shown now (`OrgLayout.assign`, set by `peopleOrg`). */
let ocLay = new Map()
const ocGov = (x) => { const l = ocLay.get(x.key); if (l) return l.band; const p = ocPlace(x); return p.governance != null ? !!p.governance : (p.influence || 0) >= 3 && (p.interest || 0) >= 3 }
const ocStream = (x) => (ocLay.get(x.key) || {}).stream || ocPlace(x).group || x.group || ''
const ocBoss = (x) => ocPlace(x).works_for || x.works_for_hint || null
const ocIdle = (x) => !!x.quiet || (!x.calls && !x.owes && !(x.facts || []).length)
function ocCard(x, d) {
  const p = ocPlace(x), thread = !S.pj.client
  const boss = ocPlace(x).works_for ? d.people.find((y) => y.key === ocPlace(x).works_for) : null
  const rep = x.reports_to ? d.people.find((y) => y.key === x.reports_to) : null
  const roleWhen = p.role && p.role_at ? `role since ${fmtDay(p.role_at)}${p.role_by === 'you' ? ' (yours)' : p.role_from ? ` (from ${p.role_from})` : ''}` : ''
  const tip = [x.title, roleWhen, rep ? `reports to ${rep.name}` : '', boss ? `works for ${boss.name} here` : x.works_for_hint ? `works for ${(d.people.find((y) => y.key === x.works_for_hint) || {}).name || '?'} here (from the work)` : ''].filter(Boolean).join(' · ')
  const meta = [x.owes ? `${x.owes} open` : '', x.late ? `<span class="late">${x.late} late</span>` : '', x.last ? `seen ${esc(relDay(x.last))}` : 'not in a call yet'].filter(Boolean).join(' · ')
  return `<button class="oc-card${S.pj.person === x.key ? ' on' : ''}" data-pp="pick" data-key="${esc(x.key)}" data-pk="${esc(x.key)}"${thread ? ` draggable="true" data-pp-drag="${esc(x.key)}" data-oc-person="${esc(x.key)}"` : ''} title="${esc(tip)}">${ppAv(x.name, p.stance)}<span class="oc-t"><b>${esc(x.name)}</b><small>${esc(p.role || x.title || '')}</small><span class="oc-m">${meta}</span></span></button>`
}
/** A workstream's people: its lead (whom most of the others work for here; else the most senior),
 *  then by seniority, then by calls; the inactive folded. */
function ocColumn(ys, key, label, d) {
  // Its lead: the one whose role leads this stream, else whom the others work for here.
  const named = (x) => (ocLay.get(x.key) || {}).lead ? 100 : 0
  const n = (x) => ys.filter((y) => ocBoss(y) === x.key).length + named(x)
  const order = ys.slice().sort((a, b) => n(b) - n(a) || (b.seniority || 0) - (a.seniority || 0) || (b.calls || 0) - (a.calls || 0) || a.name.localeCompare(b.name))
  const lead = order.length && n(order[0]) > 0 && (order.length > 1 || named(order[0])) ? order[0] : null
  const rest = order.filter((x) => x !== lead)
  const on = rest.filter((x) => !ocIdle(x)), idle = rest.filter(ocIdle)
  const open = S.pj.ocOpen && S.pj.ocOpen[key]
  const fold = idle.length ? (open ? idle.map((x) => ocCard(x, d)).join('') + `<button class="link-btn oc-more" data-pp="oc-fold" data-col="${esc(key)}">Fold</button>` : `<button class="link-btn oc-more" data-pp="oc-fold" data-col="${esc(key)}" title="${esc(idle.map((x) => x.name).join(', '))}">+${idle.length} not active</button>`) : ''
  return `<div class="oc-col" data-oc-drop="${esc(key)}"><span class="cap">${esc(label)}</span>${lead ? `<div class="oc-lead">${ocCard(lead, d)}</div>` : ''}${on.map((x) => ocCard(x, d)).join('')}${fold}${!ys.length ? '<span class="set-sub">Drop people here</span>' : ''}</div>`
}
function peopleOrg(d) {
  const people = d.people.filter((x) => x.in_thread)
  ocLay = OrgLayout.assign(people, ocPlace)
  const derived = people.filter((x) => (ocLay.get(x.key) || {}).fromRoles)
  const byOrg = new Map()
  for (const x of people) { const k = x.side === 'yours' ? '#yours' : x.org ? x.org.id : `#${x.domain || 'unknown'}`; if (!byOrg.has(k)) byOrg.set(k, []); byOrg.get(k).push(x) }
  const orgName = (k) => k === '#yours' ? 'Your company' : k.startsWith('#') ? (k.slice(1) === 'unknown' ? 'Not known' : k.slice(1)) : ((d.orgs.find((o) => o.id === k) || {}).name || k)
  const org = (k) => {
    const o = d.orgs.find((x) => x.id === k)
    const xs = byOrg.get(k) || []
    const gov = xs.filter(ocGov), work = xs.filter((x) => !ocGov(x))
    const streams = new Map()
    for (const x of work) { const g = ocStream(x); if (!streams.has(g)) streams.set(g, []); streams.get(g).push(x) }
    const cols = [...streams.entries()].sort((a, b) => (a[0] === '') - (b[0] === '') || a[0].localeCompare(b[0]))
    const rel = o ? [o.parent ? `part of ${esc((d.orgs.find((x) => x.id === o.parent) || { name: o.parent }).name)}` : '', ...(o.relations || []).map((r) => `${esc(r.kind.replace('_', ' '))} ${esc((d.orgs.find((x) => x.id === r.to) || { name: r.to }).name)}`)].filter(Boolean).join(' · ') : ''
    const band = gov.length || xs.length > 3 ? `<div class="oc-gov" data-oc-drop="${esc(k)}|gov"><span class="cap">Governance</span><div class="oc-row">${gov.slice().sort((a, b) => (b.seniority || 0) - (a.seniority || 0)).map((x) => ocCard(x, d)).join('') || '<span class="set-sub">Sponsor, partner, programme lead, PM — drop them here</span>'}</div></div>` : ''
    const fromRoles = (g) => g && xs.some((x) => ocStream(x) === g && (ocLay.get(x.key) || {}).fromRoles)
    const body = cols.map(([g, ys]) => ocColumn(ys, `${k}|${g}`, g ? `${g}${fromRoles(g) ? ' · from roles' : ''}` : (cols.length > 1 ? 'No workstream' : 'Team'), d)).join('')
    const newCol = !S.pj.client && xs.length ? (S.pj.ocNewCol === k ? `<div class="oc-col oc-new"><input class="wp-in" id="oc-new-col" data-org="${esc(k)}" placeholder="Workstream name" aria-label="New workstream" autofocus><span class="set-sub">then drop people on it</span></div>` : `<button class="oc-add" data-pp="oc-new-col" data-org="${esc(k)}" title="A new workstream column">+ Workstream</button>`) : ''
    const extra = (S.pj.ocCols || []).filter((c) => c.org === k && !streams.has(c.name)).map((c) => ocColumn([], `${k}|${c.name}`, c.name, d)).join('')
    return `<section class="pp-org oc-org${k === '#yours' ? ' yours' : ''}" data-org="${esc(o ? o.id : k)}"><div class="pp-org-h">${o ? `<button class="pp-org-n" data-pp="pick" data-key="org:${esc(o.id)}">${ppAv(o.name, (placeOf(o) || {}).stance, true)}<b>${esc(o.name)}</b></button>` : `<b>${esc(orgName(k))}</b>`}<span class="set-sub">${xs.length} ${xs.length === 1 ? 'person' : 'people'}${rel ? ` · ${rel}` : ''}</span></div>
      ${band}<div class="oc-cols">${body}${extra}${newCol}</div></section>`
  }
  // Organisations a shown one is related to (parent, partner, vendor…) are shown too, so the line has an end.
  const shown = new Set(byOrg.keys())
  for (const o of d.orgs || []) {
    if (!shown.has(o.id)) continue
    for (const t of [o.parent, ...(o.relations || []).map((r) => r.to)].filter(Boolean)) if (d.orgs.some((x) => x.id === t)) shown.add(t)
  }
  for (const o of d.orgs || []) if ((o.relations || []).some((r) => shown.has(r.to)) || (o.parent && shown.has(o.parent))) shown.add(o.id)
  // Yours on the left, the others mirrored on the right.
  const keys = [...shown].sort((a, b) => (b === '#yours') - (a === '#yours') || (a === '#unknown') - (b === '#unknown') || orgName(a).localeCompare(orgName(b)))
  // Columns read from the roles: kept as each person's workstream in one go (as dragging them would).
  const keep = derived.length && !S.pj.client ? `<button class="btn" data-pp="oc-keep" title="Save ${derived.length} people's workstream as their roles give it">Keep these columns</button>` : ''
  const bar = `<div class="oc-bar"><span class="set-sub">Who does what here. Lines: works for (solid; dashed when the work suggests it) · works with across organisations (faint)</span><span class="grow"></span>${keep}<label class="oc-tog"><input type="checkbox" data-pp-reports${S.pj.ocReports ? ' checked' : ''}> Reporting lines</label></div>`
  return keys.length ? `${bar}<div class="pp-orgs oc">${keys.map(org).join('')}</div>` : '<p class="set-sub">Nobody placed yet: add people from List.</p>'
}
/** The chart's people lines, over it: works for (solid; from the work, dashed), works with across
 *  organisations (shared calls, faint), reports to (dotted, when asked for). */
function drawPeopleLinks(wrap, d) {
  const old = wrap.querySelector(':scope > svg.oc-links')
  const box = wrap.getBoundingClientRect()
  const at = (k) => { const e = wrap.querySelector(`[data-pk="${CSS.escape(k)}"]`); if (!e || !e.offsetParent) return null; const r = e.getBoundingClientRect(); return { x: r.left - box.left + r.width / 2, y: r.top - box.top + r.height / 2, t: r.top - box.top, b: r.bottom - box.top, org: e.closest('[data-org]') } }
  const lines = []
  for (const x of d.people.filter((y) => y.in_thread)) {
    const set = ocPlace(x).works_for
    const boss = set || x.works_for_hint
    if (boss) lines.push({ a: x.key, b: boss, cls: set ? 'wf' : 'wf hint' })
    if (S.pj.ocReports && x.reports_to) lines.push({ a: x.key, b: x.reports_to, cls: 'rep' })
    for (const w of x.with || []) if (x.key < w.key) lines.push({ a: x.key, b: w.key, cls: 'ww', cross: true })
  }
  const paths = lines.map((l) => {
    const a = at(l.a), b = at(l.b)
    if (!a || !b || (l.cross && a.org === b.org)) return ''
    // From the bottom of the upper card to the top of the lower; side by side, centre to centre.
    const [p, q] = a.y <= b.y ? [{ x: a.x, y: a.b }, { x: b.x, y: b.t }] : [{ x: a.x, y: a.t }, { x: b.x, y: b.b }]
    const my = (p.y + q.y) / 2
    return `<path class="${l.cls}" d="M${p.x.toFixed(1)},${p.y.toFixed(1)} C${p.x.toFixed(1)},${my.toFixed(1)} ${q.x.toFixed(1)},${my.toFixed(1)} ${q.x.toFixed(1)},${q.y.toFixed(1)}"/>`
  }).join('')
  const key = `${paths.length}|${wrap.scrollWidth}x${wrap.scrollHeight}|${paths.slice(0, 200)}`
  if (old && old.dataset.key === key) return
  if (old) old.remove()
  if (paths) wrap.insertAdjacentHTML('afterbegin', `<svg class="oc-links" data-key="${esc(key)}" width="${wrap.scrollWidth}" height="${wrap.scrollHeight}" aria-hidden="true">${paths}</svg>`)
}
/** Lines between the org chart's organisations (drawn after the page is laid out, over it): each
 *  relationship labelled with its kind and pointing at the other side; "part of" dashed. */
function drawOrgLinks() {
  const wrap = document.querySelector('.pp-orgs')
  if (!wrap) return
  const d = peopleData()
  if (d && wrap.classList.contains('oc')) drawPeopleLinks(wrap, d)
  const edges = []
  for (const o of (d && d.orgs) || []) {
    if (o.parent) edges.push({ a: o.id, b: o.parent, kind: 'part of', dash: true })
    for (const r of o.relations || []) edges.push({ a: o.id, b: r.to, kind: r.kind.replace(/_/g, ' ') })
  }
  const key = JSON.stringify(edges) + `|${wrap.clientWidth}x${wrap.scrollHeight}`
  const old = wrap.querySelector(':scope > svg.pp-links')
  if (old && old.dataset.key === key) return
  if (old) old.remove()
  // Room for the lines between cards, before anything is measured.
  const linked = edges.some((e) => wrap.querySelector(`[data-org="${CSS.escape(e.a)}"]`) && wrap.querySelector(`[data-org="${CSS.escape(e.b)}"]`))
  wrap.classList.toggle('linked', linked)
  if (!linked) return
  const box = wrap.getBoundingClientRect()
  // Each card's side midpoints; a line joins the closest pair, so it runs card edge to card edge.
  const sides = (id) => {
    const s = wrap.querySelector(`[data-org="${CSS.escape(id)}"]`)
    if (!s) return null
    const r = s.getBoundingClientRect()
    const l = r.left - box.left, t = r.top - box.top
    return [{ x: l + r.width / 2, y: t }, { x: l + r.width / 2, y: t + r.height }, { x: l, y: t + Math.min(r.height / 2, 30) }, { x: l + r.width, y: t + Math.min(r.height / 2, 30) }]
  }
  const drawn = edges.map((e) => {
    const a = sides(e.a), b = sides(e.b)
    if (!a || !b) return null
    let best = null
    for (const p of a) for (const q of b) { const dd = Math.hypot(p.x - q.x, p.y - q.y); if (!best || dd < best.d) best = { p, q, d: dd } }
    return { ...e, p: best.p, q: best.q }
  }).filter(Boolean)
  if (!drawn.length) return
  const lines = drawn.map((e, i) => {
    const { p, q } = e
    // A curve that bows away from the cards' straight path; its label at the middle.
    const mx = (p.x + q.x) / 2, my = (p.y + q.y) / 2, dx = q.x - p.x, dy = q.y - p.y, len = Math.hypot(dx, dy) || 1
    const bow = Math.min(60, len / 4) * (i % 2 ? 1 : -1)
    const cx = mx - (dy / len) * bow, cy = my + (dx / len) * bow
    const lx = (p.x + 2 * cx + q.x) / 4, ly = (p.y + 2 * cy + q.y) / 4
    return `<path d="M${p.x.toFixed(1)},${p.y.toFixed(1)} Q${cx.toFixed(1)},${cy.toFixed(1)} ${q.x.toFixed(1)},${q.y.toFixed(1)}" class="${e.dash ? 'dash' : ''}" marker-end="url(#pp-arrow)"/>` +
      `<g class="lbl" transform="translate(${lx.toFixed(1)},${ly.toFixed(1)})"><rect x="${-(e.kind.length * 3.4 + 7)}" y="-9" width="${e.kind.length * 6.8 + 14}" height="18" rx="9"/><text y="4">${esc(e.kind)}</text></g>`
  }).join('')
  wrap.insertAdjacentHTML('beforeend', `<svg class="pp-links" data-key="${esc(key)}" width="${wrap.scrollWidth}" height="${wrap.scrollHeight}" aria-hidden="true"><defs><marker id="pp-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z"/></marker></defs>${lines}</svg>`)
}
window.addEventListener('resize', () => { if (S.view === 'projects') drawOrgLinks() })

/** The selected person or organisation, in the right panel. */
function personPanel() {
  const d = peopleData(), k = S.pj.person
  if (!d || !k) return '<div class="wp-empty">Pick someone on the People tab.</div>'
  // Its thread just deleted (or not read yet): nobody to show.
  if (!d.people || !d.orgs) return '<div class="wp-empty">Not in this view any more.</div>'
  if (k.startsWith('org:')) return orgPanel(d, k.slice(4))
  const x = d.people.find((y) => y.key === k)
  if (!x) return '<div class="wp-empty">Not in this view any more.</div>'
  const thread = S.pj.client || S.view === 'brain' ? null : S.pj.sel
  const pl = thread ? (x.places || []).find((p) => p.thread === thread) || {} : placeOf(x) || {}
  const SRC = { you: 'you', m365: 'Microsoft 365', echo: 'Echo' }
  const src = (f) => x.sources && x.sources[f] ? `<span class="set-sub"> · ${esc(SRC[x.sources[f]] || x.sources[f])}</span>` : ''
  const field = (f, label, input) => [label, S.pj.ppEdit === f ? input : `<button class="wp-val${x[f] ? '' : ' empty'}" data-pp="edit" data-f="${f}" aria-label="${esc(`${label}: ${x[f] || 'Set'}`)}">${x[f] ? esc(x[f]) : '<u>Set</u>'}</button>${src(f)}`]
  const others = d.people.filter((y) => y.key !== x.key && y.in_thread)
  const orgs = (S.pj.data && (S.pj.data.orgs || S.pj.data.clients)) || []
  // A domain no organisation has: name it (a new organisation, this thread's counterpart).
  const nameOrg = x.side === 'other' && x.domain && !x.org
    ? (S.pj.nameOrg === x.domain ? `<span class="pp-rel-add"><input class="wp-in" id="pp-org-name" placeholder="Who is ${esc(x.domain)}?" aria-label="Organisation name" autofocus><button class="btn primary" data-pp="org-name" data-domain="${esc(x.domain)}">Add</button></span>`
      : `<button class="link-btn" data-pp="org-name-ask" data-domain="${esc(x.domain)}">Name this organisation (${esc(x.domain)})</button>`) : ''
  const rows = [
    field('title', 'Title', `<input class="wp-in" data-pp-set="title" value="${esc(x.title || '')}" aria-label="Title" autofocus>`),
    ['Organisation', `<select class="wp-in" data-pp-set="org" aria-label="Organisation"><option value="">${esc(x.org && !(x.sources || {}).org ? `${x.org.name} (by mail domain)` : x.side === 'yours' ? 'Your company' : 'Not known')}</option>${orgs.map((c) => `<option value="${esc(c.id)}"${(x.sources || {}).org && x.org && x.org.id === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}</select>${nameOrg}`],
    field('group', 'Group', `<input class="wp-in" data-pp-set="group" value="${esc(x.group || '')}" list="pp-groups" aria-label="Group" autofocus><datalist id="pp-groups">${[...new Set(d.people.map((y) => y.group).filter(Boolean).concat(...d.orgs.map((o) => o.groups || [])))].map((g) => `<option value="${esc(g)}">`).join('')}</datalist>`),
    ['Reports to', `<select class="wp-in" data-pp-set="reports_to" aria-label="Reports to"><option value="">nobody set</option>${others.map((y) => `<option value="${esc(y.key)}"${x.reports_to === y.key ? ' selected' : ''}>${esc(y.name)}</option>`).join('')}</select>`],
    ['Matters', `<select class="wp-in" data-pp-set="matters" aria-label="How much they matter to you"><option value="">not set</option>${MATTERS.map((m) => `<option${x.matters === m ? ' selected' : ''}>${m}</option>`).join('')}</select>`],
    field('phone', 'Phone', `<input class="wp-in" data-pp-set="phone" value="${esc(x.phone || '')}" aria-label="Phone" autofocus>`),
    ...(x.department || x.company || x.office ? [
      field('department', 'Department', `<input class="wp-in" data-pp-set="department" value="${esc(x.department || '')}" aria-label="Department" autofocus>`),
      field('company', 'Company', `<input class="wp-in" data-pp-set="company" value="${esc(x.company || '')}" aria-label="Company" autofocus>`),
      field('office', 'Office', `<input class="wp-in" data-pp-set="office" value="${esc(x.office || '')}" aria-label="Office" autofocus>`),
    ] : []),
  ]
  const asks = (x.questions || []).map((q) => `<div class="pp-ask"><span class="grow"><b>${esc(SRC[q.source] || q.source)}</b> says ${esc(q.field)} is “${esc(q.theirs)}” — you set “${esc(q.yours || '')}”.</span><button class="btn" data-pp="answer" data-f="${esc(q.field)}" data-use="1">Use theirs</button><button class="btn ghost" data-pp="answer" data-f="${esc(q.field)}" data-use="">Keep mine</button></div>`).join('')
  const lvl = (f) => `<span class="pp-lvl" role="group" aria-label="${f}">${[1, 2, 3].map((n) => `<button data-pp="level" data-f="${f}" data-n="${n}" aria-pressed="${(pl[f] || 0) >= n}" title="${['low', 'mid', 'high'][n - 1]}"></button>`).join('')}<span class="set-sub">${pl[f] ? ['low', 'mid', 'high'][pl[f] - 1] : 'not set'}</span></span>`
  const inThread = thread ? `<div class="wp-h">In ${esc((projById(thread) || {}).name || 'this thread')}</div><div class="wp-facts">
      <span>Role</span><div>${S.pj.ppEdit === 'role' ? `<input class="wp-in" data-pp-place="role" value="${esc(pl.role || '')}" aria-label="Role" autofocus>` : `<button class="wp-val${pl.role ? '' : ' empty'}" data-pp="edit" data-f="role" aria-label="${esc(`Role: ${pl.role || 'Set'}`)}">${pl.role ? esc(pl.role) : '<u>Set</u>'}</button>${pl.role && pl.role_at ? `<div class="set-sub">${esc(fmtDay(pl.role_at))} · ${pl.role_by === 'you' ? 'yours (Echo leaves it)' : `from ${esc(pl.role_from || 'a call')}`}</div>` : ''}`}</div>
      <span>Stance</span><div><select class="wp-in" data-pp-place="stance" aria-label="Stance"><option value="">not set</option>${STANCES.map((s) => `<option${pl.stance === s ? ' selected' : ''}>${s}</option>`).join('')}</select></div>
      <span>Influence</span><div>${lvl('influence')}</div><span>Interest</span><div>${lvl('interest')}</div>
      <span>Workstream</span><div>${S.pj.ppEdit === 'group' ? `<input class="wp-in" data-pp-place="group" value="${esc(pl.group || '')}" list="pp-streams" aria-label="Workstream" autofocus><datalist id="pp-streams">${[...new Set(d.people.map((y) => ocStream(y)).filter(Boolean))].map((g) => `<option value="${esc(g)}">`).join('')}</datalist>` : `<button class="wp-val${pl.group ? '' : ' empty'}" data-pp="edit" data-f="group" aria-label="${esc(`Workstream: ${pl.group || (x.group ? `${x.group} (their group)` : 'Set')}`)}">${pl.group ? esc(pl.group) : x.group ? `${esc(x.group)} <span class="set-sub">(their group)</span>` : '<u>Set</u>'}</button>`}</div>
      <span>Governance</span><div><select class="wp-in" data-pp-place="governance" aria-label="Governance"><option value=""${pl.governance == null ? ' selected' : ''}>${ocGov(x) ? 'yes — high influence and interest' : 'no — by influence and interest'}</option><option value="yes"${pl.governance === true ? ' selected' : ''}>yes</option><option value="no"${pl.governance === false ? ' selected' : ''}>no</option></select></div>
      <span>Works for here</span><div><select class="wp-in" data-pp-place="works_for" aria-label="Works for in this thread"><option value="">${x.works_for_hint && !pl.works_for ? `${esc((d.people.find((y) => y.key === x.works_for_hint) || {}).name || '?')}? (from the work)` : 'nobody set'}</option>${d.people.filter((y) => y.key !== x.key && y.in_thread).map((y) => `<option value="${esc(y.key)}"${pl.works_for === y.key ? ' selected' : ''}>${esc(y.name)}</option>`).join('')}</select></div>
    </div>${x.in_thread ? '' : '<p class="set-sub">Not in this thread yet: setting any of these adds them.</p>'}` : `<div class="wp-h">In threads</div><div class="pp-in">${(x.places || []).map((p) => `<button class="link-btn" data-pj="goto" data-id="${esc(p.thread)}">${esc((projById(p.thread) || {}).name || p.thread)}</button>${p.role ? ` · ${esc(p.role)}` : ''}${p.stance ? ` · ${esc(p.stance)}` : ''}`).join('<br>') || '<span class="set-sub">none</span>'}</div>`
  const stat = (n, l) => `<div><b>${n}</b><span class="set-sub">${l}</span></div>`
  const hist = (x.history || []).slice().reverse().slice(0, 8).map((h) => `<div class="pp-h"><span class="set-sub">${esc(relDay(h.at))}</span> ${h.field === 'about' ? `who they are: ${h.to || h.source === 'echo' ? 'rewritten' : 'cleared'}` : `${esc(h.field)}: ${h.from ? `<s>${esc(h.from)}</s> → ` : ''}${esc(h.to || 'cleared')}`} <span class="set-sub">· ${esc(SRC[h.source] || h.source)}</span></div>`).join('')
  // Who they are (Echo's summary from what was said, or the user's own words) and what was said.
  const about = S.pj.aboutEdit != null
    ? `<textarea class="wp-in pp-about-in" id="pp-about" rows="5" maxlength="2000" aria-label="Who they are">${esc(S.pj.aboutEdit)}</textarea><div class="wp-desc-bar"><button class="btn" data-pp="about-save">Save</button><button class="btn ghost" data-pp="about-cancel">Cancel</button><span class="set-sub">⌘S saves · Esc cancels${x.about_by === 'echo' ? ' · yours from then on: Echo won\'t rewrite it' : ''}</span></div>`
    : x.about ? `<button class="pp-about" data-pp="about-edit" title="Edit">${esc(x.about)}</button><div class="set-sub">${x.about_by === 'you' ? 'In your words' : 'Echo\'s summary of what was said — click to write your own'}</div>`
    : '<button class="link-btn" data-pp="about-edit">Describe them…</button><span class="set-sub"> Echo writes this from their calls, chats and mail once it knows enough.</span>'
  const factSrc = (f) => f.kind === 'mail'
    ? (/^https:\/\//i.test(f.url || '') ? `<a class="wp-src" href="${esc(f.url)}" data-ext title="${esc(f.title || 'The mail')} — open in Outlook">✉</a>` : '<span class="wp-src">✉</span>')
    : f.call ? `<button class="wp-src" data-call="${esc(f.call)}" title="${esc(f.title || '')}">▮ ${esc(f.ts ? String(f.ts).replace(/^00:/, '').slice(0, 5) : '')}</button>` : `<span class="wp-src" title="${esc(f.title || '')}">💬</span>`
  const allFacts = x.facts || []
  const facts = allFacts.slice(0, S.pj.ppAllFacts ? 300 : 8).map((f) => `<div class="pp-fact"><div><span class="set-sub">${esc(relDay(f.date))}</span> ${esc(f.text)}${!thread && f.thread ? ` <span class="set-sub">· ${esc((projById(f.thread) || {}).name || f.thread)}</span>` : ''}</div><div class="wp-q" title="${esc(f.title || '')}">“${esc(f.quote)}” ${factSrc(f)}</div></div>`).join('')
  const withThem = (x.with || []).map((y) => { const o = personOf(y.key); return o ? `<button class="link-btn" data-pp="pick" data-key="${esc(y.key)}">${esc(o.name)}</button><span class="set-sub"> · ${y.calls} calls</span>` : '' }).filter(Boolean).join('<br>')
  return `<div class="wp pp-panel">
    <div class="pp-p-top">${ppAv(x.name, pl.stance)}<div style="min-width:0">${S.pj.ppEdit === 'name' ? `<input class="wp-in" data-pp-set="name" value="${esc(x.name)}" aria-label="Name" autofocus>` : `<button class="wp-name pp-name" data-pp="edit" data-f="name">${esc(x.name)}</button>`}<div class="set-sub">${esc([x.title, sideLabel(x)].filter(Boolean).join(' · '))}</div></div></div>
    ${(x.maybe_same || []).map((m) => `<div class="pp-ask"><span class="grow">Same person as <b>${esc(m.name)}</b>${m.email ? ` <span class="set-sub">${esc(m.email)}</span>` : ''}?</span><button class="btn" data-pp="merge" data-into="${esc(m.key)}">Merge</button></div>`).join('')}
    ${(x.addresses || []).length ? `<div class="pp-addrs">${x.addresses.map((a, i) => `<div class="pp-addr"><a class="icon-btn" href="https://teams.microsoft.com/l/chat/0/0?users=${encodeURIComponent(a.email)}" data-ext title="Chat in Teams" aria-label="Chat in Teams with ${esc(a.email)}">${ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/>')}</a><button class="icon-btn" data-copy="${esc(a.email)}" title="Copy ${esc(a.email)}" aria-label="Copy ${esc(a.email)}">${ICON('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>')}</button><span class="pp-addr-t">${esc(a.email)}<span class="set-sub"> · ${esc(a.org || a.domain || '')}</span></span>${i ? `<button class="link-btn" data-pp="split" data-email="${esc(a.email)}" title="This address is someone else">Not the same</button>` : ''}</div>`).join('')}</div>` : ''}
    <div class="pp-same">${S.pj.ppSame ? `<select class="wp-in" id="pp-same" aria-label="Same person as"><option value="">Same person as…</option>${d.people.filter((y) => y.key !== x.key).map((y) => `<option value="${esc(y.key)}">${esc(y.name)}${y.email ? ` · ${esc(y.email)}` : ''}</option>`).join('')}</select><button class="btn" data-pp="merge-pick">Merge</button><button class="btn ghost" data-pp="same-cancel">Cancel</button>` : '<button class="link-btn" data-pp="same">Same person as…</button>'}</div>
    <div class="wp-h">Who they are</div><div class="pp-about-box">${about}</div>
    ${inThread}
    ${facts ? `<div class="wp-h">What was said about them</div>${facts}${allFacts.length > 8 ? `<button class="link-btn" data-pp="all-facts">${S.pj.ppAllFacts ? 'Fewer' : `All ${allFacts.length}`}</button>` : ''}` : ''}
    ${withThem ? `<div class="wp-h">Often in calls with</div><div class="pp-in">${withThem}</div>` : ''}
    <div class="wp-h">About them</div>${asks}<div class="wp-facts">${rows.map(([l, v]) => `<span>${esc(l)}</span><div>${v}</div>`).join('')}</div>
    <div class="wp-h">What Echo saw</div>
    <div class="pp-stats">${stat(`${x.calls}<small> of ${x.of_calls}</small>`, 'calls')}${stat(`${x.share || 0}%`, 'of the talking')}${stat(x.owes, `tasks owed${x.late ? ` · <span class="late">${x.late} late</span>` : ''}`)}${stat(x.waiting, 'waiting on them')}</div>
    <div class="set-sub">${x.first ? `First seen ${esc(relDay(x.first))}, last ${esc(relDay(x.last))}` : 'Not in a call yet'}${x.chat ? ` · ${x.chat} chat message${x.chat === 1 ? '' : 's'}` : ''}</div>
    ${hist ? `<div class="wp-h">Changes</div>${hist}` : ''}
  </div>`
}
function orgPanel(d, id) {
  const o = d.orgs.find((x) => x.id === id)
  if (!o) return '<div class="wp-empty">Not in this view any more.</div>'
  const thread = S.pj.client || S.view === 'brain' ? null : S.pj.sel
  const pl = thread ? (o.places || []).find((p) => p.thread === thread) || {} : placeOf(o) || {}
  const all = ((S.pj.data && (S.pj.data.orgs || S.pj.data.clients)) || []).filter((c) => c.id !== o.id)
  const linked = thread && ((projById(thread) || {}).orgs || []).some((x) => x.client === o.id)
  const KINDS = REL_KINDS
  const lvl = (f) => `<span class="pp-lvl" role="group" aria-label="${f}">${[1, 2, 3].map((n) => `<button data-pp="org-level" data-id="${esc(o.id)}" data-f="${f}" data-n="${n}" aria-pressed="${(pl[f] || 0) >= n}" title="${['low', 'mid', 'high'][n - 1]}"></button>`).join('')}<span class="set-sub">${pl[f] ? ['low', 'mid', 'high'][pl[f] - 1] : 'not set'}</span></span>`
  return `<div class="wp pp-panel">
    <div class="pp-p-top">${ppAv(o.name, pl.stance, true)}<div style="min-width:0">${o.account === false ? `<span class="wp-name pp-name">${esc(o.name)}</span>` : `<button class="wp-name pp-name" data-pj="client" data-id="${esc(o.id)}">${esc(o.name)}</button>`}<div class="set-sub">${o.internal ? 'your company' : o.account === false ? 'organisation' : 'client'} · ${o.people} ${o.people === 1 ? 'person' : 'people'}${o.domains && o.domains.length ? ` · ${esc(o.domains.join(', '))}` : ''}</div></div></div>
    ${thread && !o.internal && (projById(thread) || {}).client !== o.id ? `<div class="set-sub">${linked ? `A counterpart of this thread · <button class="link-btn" data-pj="org-unlink" data-org="${esc(o.id)}">Unlink</button>` : `<button class="link-btn" data-pj="org-link" data-org="${esc(o.id)}">Make it this thread's counterpart</button>`}</div>` : ''}
    ${thread ? `<div class="wp-h">In ${esc((projById(thread) || {}).name || 'this thread')}</div><div class="wp-facts">
      <span>Stance</span><div><select class="wp-in" data-pp-org="${esc(o.id)}" data-f="stance" aria-label="Stance"><option value="">not set</option>${STANCES.map((s) => `<option${pl.stance === s ? ' selected' : ''}>${s}</option>`).join('')}</select></div>
      <span>Influence</span><div>${lvl('influence')}</div><span>Interest</span><div>${lvl('interest')}</div></div>` : ''}
    <div class="wp-h">Relationships</div><div class="wp-facts">
      <span>Part of</span><div><select class="wp-in" data-pp-rel="parent" data-id="${esc(o.id)}" aria-label="Part of"><option value="">nothing</option>${all.map((c) => `<option value="${esc(c.id)}"${o.parent === c.id ? ' selected' : ''}>${esc(c.name)}</option>`).join('')}</select></div>
      ${(o.relations || []).map((r) => `<span>${esc((KINDS.find(([k]) => k === r.kind) || [0, r.kind])[1])}</span><div>${esc((all.find((c) => c.id === r.to) || { name: r.to }).name)} <button class="link-btn" data-pp="rel-del" data-id="${esc(o.id)}" data-to="${esc(r.to)}">remove</button></div>`).join('')}
      <span>Add</span><div class="pp-rel-add"><select class="wp-in" id="pp-rel-kind" aria-label="Relationship">${KINDS.map(([k, l]) => `<option value="${k}">${esc(l)}</option>`).join('')}</select><select class="wp-in" id="pp-rel-to" aria-label="Organisation"><option value="">organisation…</option>${all.map((c) => `<option value="${esc(c.id)}">${esc(c.name)}</option>`).join('')}</select><button class="btn" data-pp="rel-add" data-id="${esc(o.id)}">Add</button></div>
    </div>
    <div class="wp-h">Groups</div><div class="pp-in">${(o.groups || []).map((g) => `<span class="tok">${esc(g)}</span>`).join('') || '<span class="set-sub">none yet — set a person\'s group and it appears here</span>'}</div>
  </div>`
}
PANEL_VIEWS.push({ id: 'person', label: 'Person', icon: ICON('<circle cx="12" cy="8.5" r="3.5"/><path d="M5 20a7 7 0 0 1 14 0"/>'), area: 'right', when: () => !!S.pj.person && ((S.view === 'brain' && S.br.tab === 'people') || (S.view === 'projects' && ((S.pj.client && S.pj.ctab === 'people') || (!S.pj.client && S.pj.sel && S.pj.tab === 'people')))), render: personPanel })

function peopleSend(op, extra) {
  send({ cmd: 'project', op, id: S.pj.sel, ...extra })
  setTimeout(() => (S.view === 'brain' ? loadBrainPeople() : loadPeople()).then(render), 250)
}
function aboutSave() {
  const i = $('#pp-about')
  if (!i) return
  const v = i.value.trim()
  S.pj.aboutEdit = null
  personSet('about', v)
}
/** Change… on a suggestion: its row becomes your version, focused. */
function sugEdit(sid) {
  S.pj.ppSugEdit = sid
  render()
  const i = $('#pp-sug-v')
  if (i) { i.focus(); i.select && i.select() }
}
function sugSave(thread, sid) {
  const i = $('#pp-sug-v'), value = i ? i.value.trim() : ''
  if (!value) return window.echo.notice('Say what it should be, or press No.')
  S.pj.ppSugEdit = null
  send({ cmd: 'project', op: 'people_decide', id: thread, sid, yes: true, value })
  setTimeout(() => { loadPeople().then(render); loadAttention(true).then(render) }, 250)
}
function personOf(key) { const d = peopleData(); return d && d.people.find((y) => y.key === key) }
function personSet(field, value) {
  const x = personOf(S.pj.person)
  if (!x) return
  // The directory's group list follows a person's group (the organisation keeps the names).
  if (field === 'group' && value && x.org) {
    const o = ((S.pj.people || {}).orgs || []).find((y) => y.id === x.org.id)
    if (o && !(o.groups || []).includes(value)) send({ cmd: 'project', op: 'client_set', id: o.id, groups: [...(o.groups || []), value] })
  }
  S.pj.ppEdit = null
  peopleSend('person_set', { key: x.key, name: x.name, email: x.email, field, value })
}
function personPlace(key, set) {
  const x = personOf(key)
  if (x) return peopleSend('people_place', { name: x.name, email: x.email, set })
  if (key.startsWith('org:')) peopleSend('org_place', { client: key.slice(4), set })
}
function showPerson(key) {
  S.pj.person = key
  S.pj.ppEdit = null
  const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (eff.narrow) S.overlay = true
  Layout.show('person', known)
  layoutChanged()
}
document.addEventListener('click', (e) => {
  const b = e.target.closest && e.target.closest('[data-pp]')
  if (!b) return
  e.preventDefault()
  const d = b.dataset
  switch (d.pp) {
    case 'view': try { localStorage.setItem('echo.people.view', d.v) } catch { /* storage off */ } return render()
    case 'pick': S.pj.aboutEdit = null; return showPerson(d.key)
    case 'about-edit': { const x = personOf(S.pj.person); S.pj.aboutEdit = (x && x.about) || ''; render(); const i = $('#pp-about'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length) } return }
    case 'about-cancel': S.pj.aboutEdit = null; return render()
    case 'about-save': return aboutSave()
    case 'all-facts': S.pj.ppAllFacts = !S.pj.ppAllFacts; return render()
    case 'oc-keep': { const d2 = peopleData(); if (!d2) return; for (const x of d2.people.filter((y) => y.in_thread && (ocLay.get(y.key) || {}).fromRoles)) personPlace(x.key, { group: ocLay.get(x.key).stream }); return }
    case 'oc-fold': S.pj.ocOpen = { ...(S.pj.ocOpen || {}), [d.col]: !(S.pj.ocOpen || {})[d.col] }; return render()
    case 'oc-new-col': S.pj.ocNewCol = d.org; render(); { const i = $('#oc-new-col'); if (i) i.focus() } return
    case 'org-name-ask': S.pj.nameOrg = d.domain; render(); { const i = $('#pp-org-name'); if (i) i.focus() } return
    case 'org-name': { const name = ($('#pp-org-name') || {}).value || ''; if (!name.trim()) return; S.pj.nameOrg = null; send({ cmd: 'project', op: 'org_name', id: S.pj.client || S.view === 'brain' ? '' : S.pj.sel || '', name: name.trim(), domain: d.domain }); return setTimeout(async () => { await loadProjects(); await loadPeople(); render() }, 300) }
    case 'edit': S.pj.ppEdit = d.f; render(); { const i = document.querySelector('[data-pp-set][autofocus],[data-pp-place][autofocus]'); if (i) { i.focus(); i.select && i.select() } } return
    case 'level': { const x = personOf(S.pj.person), pl = x && (x.places || []).find((p) => p.thread === S.pj.sel); const cur = pl && pl[d.f]; return personPlace(S.pj.person, { [d.f]: cur === Number(d.n) ? '' : Number(d.n) }) }
    case 'org-level': { const o = ((S.pj.people || {}).orgs || []).find((y) => y.id === d.id), pl = o && (o.places || []).find((p) => p.thread === S.pj.sel); const cur = pl && pl[d.f]; return peopleSend('org_place', { client: d.id, set: { [d.f]: cur === Number(d.n) ? '' : Number(d.n) } }) }
    case 'merge': case 'merge-pick': {
      const x = personOf(S.pj.person), into = d.pp === 'merge' ? d.into : ($('#pp-same') || {}).value, y = into && personOf(into)
      if (!x || !y) return
      S.pj.ppSame = false
      // The one in the thread (else the one seen more) stays; the other's addresses join it.
      const keep = (y.in_thread && !x.in_thread) || (y.in_thread === x.in_thread && y.calls >= x.calls) ? y : x, other = keep === y ? x : y
      S.pj.person = keep.key
      return peopleSend('person_merge', { key: other.key, key_name: other.name, key_emails: [other.email, ...(other.also || [])].filter(Boolean), into: keep.key, into_name: keep.name, into_email: keep.email })
    }
    case 'same': S.pj.ppSame = true; return render()
    case 'same-cancel': S.pj.ppSame = false; return render()
    case 'split': { const x = personOf(S.pj.person); if (x) peopleSend('person_split', { key: x.key, name: x.name, email: d.email }); return }
    case 'decide': send({ cmd: 'project', op: 'people_decide', id: d.thread, sid: d.sid, yes: !!d.yes }); return setTimeout(() => loadPeople().then(render), 250)
    case 'sug-edit': return sugEdit(d.sid)
    case 'sug-cancel': S.pj.ppSugEdit = null; return render()
    case 'sug-save': return sugSave(d.thread, d.sid)
    case 'all-sug': S.pj.ppAllSug = !S.pj.ppAllSug; return render()
    case 'answer': { const x = personOf(S.pj.person); if (x) peopleSend('person_answer', { key: x.key, field: d.f, use: !!d.use }); return }
    case 'add': S.pj.ppAdding = true; render(); { const i = $('#pp-new'); if (i) i.focus() } return
    case 'add-all': { const dd = peopleData(); return peopleSend('people_add', { people: dd.people.filter((x) => !x.in_thread).map((x) => ({ name: x.name, email: x.email })) }) }
    case 'add-one': { const x = personOf(d.key); if (x) peopleSend('people_add', { people: [{ name: x.name, email: x.email }] }); return }
    case 'rel-add': { const kind = $('#pp-rel-kind').value, to = $('#pp-rel-to').value; if (!to) return; send({ cmd: 'project', op: 'client_set', id: d.id, relation: { to, kind } }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250) }
    case 'rel-del': send({ cmd: 'project', op: 'client_set', id: d.id, relation: { to: d.to, kind: '' } }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250)
  }
})
document.addEventListener('change', (e) => {
  const t = e.target
  if (!t.matches) return
  if (t.matches('[data-pp-set]') && t.tagName === 'SELECT') return personSet(t.dataset.ppSet, t.value)
  if (t.matches('[data-pp-place="governance"]')) return personPlace(S.pj.person, { governance: t.value === 'yes' ? true : t.value === 'no' ? false : null })
  if (t.matches('[data-pp-place]') && t.tagName === 'SELECT') return personPlace(S.pj.person, { [t.dataset.ppPlace]: t.value })
  if (t.matches('[data-pp-org]')) return peopleSend('org_place', { client: t.dataset.ppOrg, set: { [t.dataset.f]: t.value } })
  if (t.matches('[data-pp-rel="parent"]')) { send({ cmd: 'project', op: 'client_set', id: t.dataset.id, parent: t.value }); return setTimeout(() => loadProjects().then(loadPeople).then(render), 250) }
})
document.addEventListener('focusout', (e) => {
  const t = e.target
  if (!t.matches || t.tagName !== 'INPUT') return
  if (t.matches('[data-pp-set]')) return personSet(t.dataset.ppSet, t.value)
  if (t.matches('[data-pp-place]')) { S.pj.ppEdit = null; return personPlace(S.pj.person, { [t.dataset.ppPlace]: t.value }) }
  if (t.id === 'pp-new' && !t.value.trim()) { S.pj.ppAdding = false; render() }
})
document.addEventListener('keydown', (e) => {
  const t = e.target
  if (!t.matches) return
  if ((t.matches('[data-pp-set]') || t.matches('[data-pp-place]')) && t.tagName === 'INPUT') {
    if (e.key === 'Enter') { e.preventDefault(); t.blur() }
    if (e.key === 'Escape') { e.preventDefault(); S.pj.ppEdit = null; render() }
  }
  if (t.id === 'pp-new' && e.key === 'Enter') {
    e.preventDefault()
    const m = t.value.trim().match(/^(.*?)\s*<([^>]+@[^>]+)>$/) || t.value.trim().match(/^()([^\s]+@[^\s]+)$/)
    const name = m ? (m[1] || m[2].split('@')[0]) : t.value.trim(), email = m ? m[2] : undefined
    S.pj.ppAdding = false
    if (name) peopleSend('people_add', { people: [{ name, email }] })
    else render()
  }
  if (t.id === 'pp-new' && e.key === 'Escape') { S.pj.ppAdding = false; render() }
  if (t.id === 'pp-about') {
    if (e.key === 'Escape') { e.preventDefault(); S.pj.aboutEdit = null; return render() }
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 's') { e.preventDefault(); return aboutSave() }
  }
  if (t.id === 'pp-sug-v' && e.key === 'Enter') { e.preventDefault(); const b = document.querySelector('[data-pp="sug-save"]'); if (b) b.click() }
  if (t.id === 'pp-sug-v' && e.key === 'Escape') { e.preventDefault(); S.pj.ppSugEdit = null; render() }
})
// The org chart: a new workstream column (kept in the page until someone is dropped on it), reporting lines.
document.addEventListener('keydown', (e) => {
  const t = e.target
  if (t.id !== 'oc-new-col') return
  if (e.key === 'Escape') { S.pj.ocNewCol = null; return render() }
  if (e.key === 'Enter' && t.value.trim()) { S.pj.ocCols = [...(S.pj.ocCols || []), { org: t.dataset.org, name: t.value.trim() }]; S.pj.ocNewCol = null; render() }
})
document.addEventListener('change', (e) => { if (e.target.matches && e.target.matches('[data-pp-reports]')) { S.pj.ocReports = e.target.checked; render() } })
// Drag a card on the org chart: onto a person (works for them here), a column (that workstream), the
// governance band.
document.addEventListener('dragover', (e) => {
  const q = e.target.closest && (e.target.closest('[data-oc-person]') || e.target.closest('[data-oc-drop]'))
  if (q && [...e.dataTransfer.types].includes('text/x-echo-person')) { e.preventDefault(); document.querySelectorAll('.oc .over').forEach((x) => x !== q && x.classList.remove('over')); q.classList.add('over') }
})
document.addEventListener('drop', (e) => {
  const card = e.target.closest && e.target.closest('[data-oc-person]')
  const col = e.target.closest && e.target.closest('[data-oc-drop]')
  const key = e.dataTransfer.getData('text/x-echo-person')
  if (!key || !(card || col)) return
  e.preventDefault()
  document.querySelectorAll('.oc .over').forEach((x) => x.classList.remove('over'))
  if (card && card.dataset.ocPerson !== key) return personPlace(key, { works_for: card.dataset.ocPerson })
  if (!col || card) return
  const [, g] = col.dataset.ocDrop.split('|')
  personPlace(key, g === 'gov' ? { governance: true } : { governance: false, group: g || '' })
})
// Drag a chip to a quadrant: high or low influence and interest.
document.addEventListener('dragstart', (e) => { const c = e.target.closest && e.target.closest('[data-pp-drag]'); if (c) { e.dataTransfer.setData('text/x-echo-person', c.dataset.ppDrag); e.dataTransfer.effectAllowed = 'move' } })
document.addEventListener('dragover', (e) => { const q = e.target.closest && e.target.closest('[data-pp-drop]'); if (q && [...e.dataTransfer.types].includes('text/x-echo-person')) { e.preventDefault(); q.classList.add('over') } })
document.addEventListener('dragleave', (e) => { const q = e.target.closest && e.target.closest('[data-pp-drop]'); if (q && !q.contains(e.relatedTarget)) q.classList.remove('over') })
document.addEventListener('drop', (e) => {
  const q = e.target.closest && e.target.closest('[data-pp-drop]')
  const key = e.dataTransfer.getData('text/x-echo-person')
  if (!q || !key) return
  e.preventDefault()
  q.classList.remove('over')
  const [inf, int] = q.dataset.ppDrop === 'tray' ? ['', ''] : q.dataset.ppDrop.split('/').map(Number)
  personPlace(key, { influence: inf, interest: int })
})

