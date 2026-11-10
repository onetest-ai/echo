// ── People: stakeholders around a thread or a client (`people.rs`) ────────────────────────
// Facts from the calls (computed by the app every time) beside what the user set: a Map
// (influence × interest), the org chart, a List; the person in the right panel.
const STANCES = ['champion', 'supporter', 'neutral', 'sceptic', 'blocker']
/** Relationships between organisations (`projects::RELATIONS`), as a sentence reads them. */
const REL_KINDS = [['partner', 'partner of'], ['vendor', 'vendor to'], ['delivers_for', 'delivers for'], ['integrates_for', 'integrates for'], ['customer', 'customer of'], ['competitor', 'competitor of']]
const MATTERS = ['critical', 'high', 'normal', 'irrelevant']
const peopleData = () => { if (S.view === 'brain') return S.br.people && S.br.people.people ? S.br.people : null; const d = S.pj.people; const want = S.pj.client ? `api/client/${encodeURIComponent(S.pj.client)}/people` : `api/project/${encodeURIComponent(S.pj.sel)}/people`; return d && d.url === want ? d : null }
const peopleCount = () => { const d = peopleData(); return d && d.people ? d.people.filter((x) => x.in_thread).length : 0 }
const ppView = () => { try { return localStorage.getItem('echo.people.view') || 'map' } catch { return 'map' } }
const initials = (n) => (n || '?').split(/[\s@._-]+/).filter(Boolean).slice(0, 2).map((w) => w[0].toUpperCase()).join('')
/** Where a person or organisation stands in this view: the thread's own place; on a client, the
 *  thread where they weigh most. */
function placeOf(x) {
  const ps = (x.places || []).filter((p) => p.influence || p.interest || p.stance || p.role)
  if (!ps.length) return x.places && x.places[0] ? x.places[0] : null
  return ps.slice().sort((a, b) => (b.influence || 0) - (a.influence || 0))[0]
}
const ppAv = (name, stance, org) => `<span class="pp-av${org ? ' org' : ''} ${esc(stance || 'none')}" aria-hidden="true">${esc(initials(name))}</span>`
const sideLabel = (x) => x.side === 'yours' ? 'your company' : x.org ? x.org.name : x.domain || 'outside'
function peopleView() {
  const d = peopleData()
  if (!d) return `<div class="skeleton">${'<i></i>'.repeat(6)}</div>`
  if (d.error) return `<p class="error">${esc(d.error)}</p>`
  const v = ppView()
  const legend = STANCES.map((s) => `<span class="pp-leg"><i class="pp-av ${s}"></i>${s}</span>`).join('')
  const seg = `<div class="pp-seg" role="group" aria-label="View">${[['map', 'Map'], ['org', 'Org chart'], ['list', 'List']].map(([k, l]) => `<button data-pp="view" data-v="${k}" aria-pressed="${v === k}">${l}</button>`).join('')}</div>`
  const add = S.pj.client ? '' : S.pj.ppAdding ? `<span class="tok-in"><input id="pp-new" placeholder="Name, or name &lt;email&gt;" aria-label="New person" autofocus></span>` : '<button class="btn" data-pp="add">+ Person</button>'
  const bar = `<div class="pp-bar">${seg}<span class="grow"></span>${legend}${add}</div>`
  const body = v === 'list' ? peopleList(d) : v === 'org' ? peopleOrg(d) : peopleMap(d)
  return `<div class="pp">${bar}${peopleSuggestions(d)}${body}</div>`
}
/** What the sync's people pass proposed, with the words: Yes applies it as yours, No turns it down
 *  (never proposed again), Change… applies your own version instead (the proposed one stays turned
 *  down). Three at a time; the one being changed is always shown. */
function peopleSuggestions(d) {
  const all = d.suggestions || []
  if (!all.length) return ''
  const LV = { 1: 'low', 2: 'mid', 3: 'high' }
  const ask = (x) => {
    const n = `<b>${esc(x.target.name || '')}</b>`, v = esc(x.value)
    switch (x.field) {
      case 'stance': return `Is ${n} a <b>${v}</b>?`
      case 'role': return `${n}'s role here: <b>${v}</b>?${x.was ? ` <span class="set-sub">(was “${esc(x.was)}”)</span>` : ''}`
      case 'person': return `${n}${x.target.org_name ? ` (${esc(x.target.org_name)})` : ''} — mentioned${x.at && x.at.title ? ` in ${esc(x.at.title)}` : ''}${(x.mentions || 1) > 1 ? `, ${x.mentions} times` : ''}${x.role ? `, ${esc(x.role)}` : ''}: add?`
      case 'influence': case 'interest': return `${n}'s ${x.field}: <b>${LV[x.value] || v}</b>?`
      case 'title': return `${n} is <b>${v}</b>?`
      case 'group': return `${n} is in <b>${v}</b>?`
      case 'reports_to': return `${n} reports to <b>${esc(x.value_name || x.value)}</b>?`
      case 'left': return `Has ${n} left?`
      case 'relation': return `${n} <b>${esc(x.value.replace('_', ' '))}</b> ${esc(x.target.to_name || '')}?`
    }
    return `${n}: ${esc(x.field)} = <b>${v}</b>?`
  }
  /** The control for your own version: the field's choices, else the proposed text to correct. */
  const editor = (x) => {
    const sel = (opts) => `<select id="pp-sug-v" aria-label="Your version">${opts.map(([v, l]) => `<option value="${esc(v)}"${String(v) === String(x.value) ? ' selected' : ''}>${esc(l)}</option>`).join('')}</select>`
    switch (x.field) {
      case 'stance': return sel(STANCES.map((v) => [v, v]))
      case 'influence': case 'interest': return sel([1, 2, 3].map((n) => [n, LV[n]]))
      case 'relation': return sel(REL_KINDS)
      case 'reports_to': return sel(d.people.filter((p) => p.key !== x.target.key).map((p) => [p.key, p.name]))
    }
    return `<input id="pp-sug-v" value="${esc(x.value)}" maxlength="80" aria-label="Your version" autofocus>`
  }
  const row = (x) => {
    const at = x.at || {}
    const src = [at.title, at.date ? relDay(at.date) : '', at.ts ? String(at.ts).replace(/^00:/, '') : ''].filter(Boolean).join(' · ')
    const st = x.field === 'stance' ? x.value : null
    if (S.pj.ppSugEdit === x.sid) {
      return `<div class="pp-sug" role="listitem">${ppAv(x.target.name || '?', st, x.field === 'relation')}<span class="grow">${ask(x).replace(/\?$/, '')} → ${editor(x)} <span class="pp-quote">“${esc(x.quote || '')}”</span></span><button class="btn primary" data-pp="sug-save" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}">Save</button><button class="btn ghost" data-pp="sug-cancel">Cancel</button></div>`
    }
    return `<div class="pp-sug" role="listitem">${ppAv(x.target.name || '?', st, x.field === 'relation')}<span class="grow">${ask(x)} <span class="pp-quote">“${esc(x.quote || '')}”</span> ${at.call ? `<button class="link-btn pp-src" data-call="${esc(at.call)}">${esc(src)}</button>` : /^https:\/\//i.test(at.url || '') ? `<a class="link-btn pp-src" href="${esc(at.url)}" data-ext title="Open the mail in Outlook">✉ ${esc(src)}</a>` : `<span class="set-sub">${esc(src)}</span>`}</span><button class="btn primary" data-pp="decide" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}" data-yes="1">Yes</button><button class="btn ghost" data-pp="decide" data-sid="${esc(x.sid)}" data-thread="${esc(x.thread)}" data-yes="">No</button>${x.field === 'left' ? '' : `<button class="btn ghost" data-pp="sug-edit" data-sid="${esc(x.sid)}" title="Not quite: put it right">Change…</button>`}</div>`
  }
  const shown = S.pj.ppAllSug || all.slice(3).some((x) => x.sid === S.pj.ppSugEdit) ? all : all.slice(0, 3)
  return `<div class="pp-sugs" role="list" aria-label="Suggestions">${shown.map(row).join('')}${all.length > 3 ? `<button class="link-btn" data-pp="all-sug">${S.pj.ppAllSug ? 'Show fewer' : `${all.length - 3} more from the calls`}</button>` : ''}</div>`
}
function peopleMap(d) {
  const thread = !S.pj.client
  const people = d.people.filter((x) => x.in_thread)
  const orgs = d.orgs.filter((o) => (o.places || []).length)
  const items = [...people.map((x) => ({ x, org: false, pl: placeOf(x) })), ...orgs.map((o) => ({ x: o, org: true, pl: placeOf(o) }))]
  const hi = (n) => (n || 0) >= 2
  const placed = items.filter((i) => i.pl && i.pl.influence && i.pl.interest)
  const loose = items.filter((i) => !(i.pl && i.pl.influence && i.pl.interest))
  const chip = (i) => {
    const key = i.org ? `org:${i.x.id}` : i.x.key
    const sub = i.org ? (i.x.internal ? 'your company' : 'organisation') : (i.pl && i.pl.role) || i.x.title || sideLabel(i.x)
    return `<button class="pp-chip${S.pj.person === key ? ' on' : ''}${i.x.new ? ' new' : ''}" data-pp="pick" data-key="${esc(key)}"${thread ? ` draggable="true" data-pp-drag="${esc(key)}"` : ''} title="${esc(i.x.name)}">${ppAv(i.x.name, i.pl && i.pl.stance, i.org)}<span class="pp-cn">${esc(i.x.name)}</span><small>${esc(sub)}</small>${i.x.new ? '<span class="pp-new">new</span>' : ''}${i.x.quiet ? '<span class="pp-quiet" title="Not seen for six weeks while the calls went on">quiet</span>' : ''}</button>`
  }
  const Q = [['3', '1', 'Keep satisfied', 'high influence · low interest'], ['3', '3', 'Manage closely', 'high influence · high interest'], ['1', '1', 'Monitor', 'low influence · low interest'], ['1', '3', 'Keep informed', 'low influence · high interest']]
  const quad = ([inf, int, t, sub]) => {
    const xs = placed.filter((i) => hi(i.pl.influence) === (inf === '3') && hi(i.pl.interest) === (int === '3'))
    return `<section class="pp-q${inf === '3' && int === '3' ? ' key' : ''}" data-pp-drop="${inf}/${int}" aria-label="${esc(t)}"><div class="pp-qh"><b>${esc(t)}</b><span class="set-sub">${esc(sub)}</span></div><div class="pp-chips">${xs.map(chip).join('')}</div></section>`
  }
  const seenOnly = d.people.filter((x) => !x.in_thread).length
  const tray = loose.length ? `<div class="pp-tray" data-pp-drop="tray" role="group" aria-label="Not placed yet"><span class="cap">Not placed yet</span>${loose.map(chip).join('')}</div>` : ''
  const empty = !items.length ? `<div class="wk-empty"><b>Nobody in ${thread ? 'this thread' : "this client's threads"} yet</b><p>${seenOnly ? `${seenOnly} people were in its calls — add them from <button class="link-btn" data-pp="view" data-v="list">List</button>, then drag them where they stand.` : 'People join as its calls come in, or add someone with + Person.'}</p></div>` : ''
  return `${empty}${items.length ? `<div class="pp-grid"><div class="pp-axis y"><span>Influence →</span></div>${Q.map(quad).join('')}<span></span><div class="pp-axis x"><span>Interest →</span></div></div>${tray}` : ''}
    ${seenOnly ? `<p class="set-sub">${thread ? 'Drag a person to another quadrant. ' : ''}${seenOnly} more ${seenOnly === 1 ? 'was' : 'were'} in the calls and ${seenOnly === 1 ? "isn't" : "aren't"} in ${thread ? 'this thread' : 'a thread'} yet — <button class="link-btn" data-pp="view" data-v="list">List</button>.</p>` : ''}`
}
function peopleList(d) {
  const thread = !S.pj.client
  const row = (x) => {
    const pl = placeOf(x)
    const threads = !thread ? (x.places || []).map((p) => projById(p.thread)).filter(Boolean).map((p) => `<span class="tk-pj">${projIcon(p, 12)}${esc(p.name)}</span>`).join('') : ''
    return `<tr class="${S.pj.person === x.key ? 'on' : ''}" data-pp="pick" data-key="${esc(x.key)}">
      <td><span class="pp-who">${ppAv(x.name, pl && pl.stance)}<span><span class="pp-nm"><b>${esc(x.name)}</b>${x.new ? ' <span class="pp-new">new</span>' : ''}${x.quiet ? ' <span class="pp-quiet">quiet</span>' : ''}</span><span class="set-sub">${esc(x.title || x.email || '')}</span></span></span></td>
      <td>${esc(sideLabel(x))}${x.group ? `<span class="set-sub"> · ${esc(x.group)}</span>` : ''}</td>
      <td>${thread ? esc((pl && pl.role) || '') : threads}</td>
      <td>${pl && pl.stance ? `<span class="pp-leg"><i class="pp-av ${esc(pl.stance)}"></i>${esc(pl.stance)}</span>` : ''}</td>
      <td class="num">${x.calls}<span class="set-sub">/${x.of_calls}</span></td>
      <td class="num">${x.share ? `${x.share}%` : ''}</td>
      <td class="num">${x.owes ? `${x.owes}${x.late ? ` <span class="late">${x.late} late</span>` : ''}` : ''}</td>
      <td>${x.last ? esc(relDay(x.last)) : ''}</td>
      <td class="pp-act">${x.email ? `<a class="icon-btn" href="https://teams.microsoft.com/l/chat/0/0?users=${encodeURIComponent(x.email)}" data-ext title="Chat in Teams" aria-label="Chat with ${esc(x.name)} in Teams">${ICON('<path d="M5 18.5V7a2.5 2.5 0 0 1 2.5-2.5h9A2.5 2.5 0 0 1 19 7v6a2.5 2.5 0 0 1-2.5 2.5H9z"/>')}</a><button class="icon-btn" data-copy="${esc(x.email)}" title="Copy ${esc(x.email)}" aria-label="Copy ${esc(x.name)}'s email">${ICON('<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 7l9 6 9-6"/>')}</button>` : ''}</td>
    </tr>`
  }
  const inn = d.people.filter((x) => x.in_thread), seen = d.people.filter((x) => !x.in_thread)
  const head = `<thead><tr><th>Person</th><th>Organisation</th><th>${thread ? 'Role' : 'Threads'}</th><th>Stance</th><th class="num">Calls</th><th class="num">Talk</th><th class="num">Owes</th><th>Last seen</th><th><span class="sr">Contact</span></th></tr></thead>`
  const seenBox = seen.length ? `<div class="pp-seen"><div class="pp-seen-h"><span class="cap">Seen in calls</span><span class="set-sub">${seen.length} ${seen.length === 1 ? "isn't" : "aren't"} in ${thread ? 'this thread' : 'a thread'} yet</span><span class="grow"></span>${thread ? `<button class="btn" data-pp="add-all">Add all</button>` : ''}</div>
    <table class="pp-tbl" aria-label="Seen in calls">${head}<tbody>${seen.map((x) => row(x).replace('<td class="pp-act">', `<td class="pp-act">${thread ? `<button class="btn ghost" data-pp="add-one" data-key="${esc(x.key)}">Add</button>` : ''}`)).join('')}</tbody></table></div>` : ''
  return `${inn.length ? `<div class="pp-card"><table class="pp-tbl" aria-label="${thread ? 'In this thread' : "In this client's threads"}">${head}<tbody>${inn.map(row).join('')}</tbody></table></div>` : `<p class="set-sub">Nobody in ${thread ? 'this thread' : "this client's threads"} yet.</p>`}${seenBox}`
}
/** The org chart (#43): who does what in this engagement, not who reports to whom (those lines are
 *  rarely known). Each organisation — yours first, then the others — has a governance band on top
 *  (sponsor, partner, programme lead, PM: marked, else high influence and high interest), then a
 *  column per workstream (the person's `group` in the thread), its lead on top — the one the others
 *  work for here — and the rest by title seniority; people inactive in the thread folded ("+N").
 *  Reporting lines are metadata: the card's tooltip, the panel, faint lines when asked for. In a
 *  thread, drag a card to a column (its workstream), to the band (governance), or onto a person
 *  (works for them here). */
