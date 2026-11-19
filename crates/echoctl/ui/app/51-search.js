// ── search: what was said, from the Calls list's box ────────────────────────────────────────
// Enter in the box (or ⌘K's "Search what was said…") turns the list into results (`/api/search`): the calls whose
// title or people match, then what was said in calls, mail, channels and documents, each source with its moments. A
// moment opens in the right panel's "What was said" (`/api/search/moment`). With Intake off the box keeps to titles
// and people, as before. Others' words reach the page only through `esc` (`sxMark`).

// ── search request: filters, chips and labels (no DOM: `search.test.mjs`) ─────────────────
/** The source kinds: id, name (the icon's tooltip), as said in "Said in …", the icon. */
const SX_KINDS = [
  ['call', 'Calls', 'calls', '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>'],
  ['mail', 'Mail', 'mail', '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3.5 7l8.5 6 8.5-6"/>'],
  ['channel', 'Teams channels', 'channels', '<path d="M5 9h14M5 15h14M10 4L8 20M16 4l-2 16"/>'],
  ['document', 'Documents', 'documents', '<path d="M6 3.5h8l4 4v13H6z"/><path d="M14 3.5V8h4"/>'],
]
/** The sections the filter offers (the index's section kinds that aren't a source kind), and every section's name. */
const SX_SECTIONS = [['summary', 'Summary'], ['decisions', 'Decisions'], ['action_items', 'Action items'], ['open_questions', 'Open questions'], ['transcript', 'Transcript'], ['chat', 'Chat']]
const SX_SECTION_NAMES = Object.fromEntries([...SX_SECTIONS, ['minutes', 'Minutes'], ['mail', 'Mail'], ['channel', 'Post'], ['document', 'Document'], ['card', 'Call card']])
/** No filter: every kind, no section, client, thread, person or days. */
const sxNoFilters = () => ({ kinds: [], sections: [], client: null, thread: null, person: null, since: null, until: null })
/** The request for `/api/search`: the query and each filter that is set. */
function sxRequest(f, q) {
  const p = [['q', q], ['kind', f.kinds.join(',')], ['section', f.sections.join(',')], ['client', f.client], ['thread', f.thread], ['person', f.person], ['since', f.since], ['until', f.until]]
  return p.filter(([, v]) => v).map(([k, v]) => `${k}=${encodeURIComponent(v)}`).join('&')
}
/** The group of what was said: "Said in calls, mail, channels" (every kind), or the kinds picked. */
function sxSaidLabel(kinds) {
  const picked = SX_KINDS.filter((k) => kinds.includes(k[0]))
  return `Said in ${(picked.length ? picked : SX_KINDS.slice(0, 3)).map((k) => k[2]).join(', ')}`
}
/** Days as a chip says them: "Since Nov 1", "Until Jul 31", "Nov 12", "Nov 1 – Nov 12" (`day` names a day, `today` is one). */
function sxDays(since, until, day, today) {
  if (since && until) return since === until ? day(since) : until === today ? `Since ${day(since)}` : `${day(since)} – ${day(until)}`
  return since ? `Since ${day(since)}` : until ? `Until ${day(until)}` : ''
}
/** The active filters as chips, in the row's order: sections, client, thread, person, days. A date read from the
 *  query's words is a guess (dashed), its words in the tooltip; removing it takes them out of the query. `name(kind, id)`
 *  names a client or thread. Each: {key, label, guess, title}. */
function sxChips(f, res, name, day, today) {
  const out = f.sections.map((s) => ({ key: `section:${s}`, label: SX_SECTION_NAMES[s] || s }))
  if (f.client) out.push({ key: 'client', label: name('client', f.client) })
  if (f.thread) out.push({ key: 'thread', label: name('thread', f.thread) })
  if (f.person) out.push({ key: 'person', label: `Said by ${f.person}` })
  if (f.since || f.until) out.push({ key: 'days', label: sxDays(f.since, f.until, day, today) })
  else if (res && res.filters && res.filters.dates_from_query) {
    const r = res.filters
    out.push({ key: 'guess', label: sxDays(r.since, r.until, day, today), guess: true, title: `Read from “${r.dates_from_query}” in your words` })
  }
  return out
}
/** The query without the words a date was read from (any case), its spaces tidied. */
function sxDropWords(q, words) {
  const at = words ? q.toLowerCase().indexOf(words.toLowerCase()) : -1
  return (at < 0 ? q : q.slice(0, at) + q.slice(at + words.length)).replace(/\s+/g, ' ').trim()
}
/** `text` as HTML: every word looked for (whole, any case) in <mark>; everything through `esc`. */
function sxMark(text, words, esc) {
  const want = new Set((words || []).map((w) => String(w).toLowerCase()))
  if (!want.size) return esc(text)
  // Words marked next to each other ("SSO cutover") are one mark.
  return String(text ?? '').split(/([\p{L}\p{N}]+)/u).map((part, i) => (i % 2 && want.has(part.toLowerCase()) ? `<mark>${esc(part)}</mark>` : esc(part))).join('').replace(/<\/mark>( +)<mark>/g, '$1')
}
/** The sources shown open: those the user opened or closed (`toggled`: source → open), else all of two or fewer, else the first. */
function sxOpen(sources, toggled) {
  return new Set(sources.filter((s, i) => (s.source in toggled ? toggled[s.source] : sources.length <= 2 || i === 0)).map((s) => s.source))
}
/** A moment's section as its line says it. */
const sxSection = (m) => (m.section_kind === 'minutes' || m.section_kind === 'document') && m.heading ? m.heading : SX_SECTION_NAMES[m.section_kind] || m.heading || ''
/** When a moment was said: its time in the call (`clock`), else its day (`day`). */
const sxWhen = (m, clock, day) => (m.t != null ? clock(m.t) : m.day ? day(m.day) : '')
/** "No match", as one line: what was searched (the client's or thread's, the kinds) and whether a wider search exists. */
function sxNothing(f, q, name) {
  const whose = f.thread ? name('thread', f.thread) : f.client ? name('client', f.client) : ''
  const kinds = SX_KINDS.filter((k) => f.kinds.includes(k[0])).map((k) => k[2])
  const narrowed = whose || kinds.length || f.sections.length || f.person || f.since || f.until
  if (!whose && !kinds.length) return { text: `Nothing said matches “${q}”.`, wider: !!narrowed }
  return { text: `Nothing in ${whose ? `${whose}’s ` : ''}${kinds.length ? kinds.join(' or ') : 'sources'} says “${q}”.`, wider: !!narrowed }
}
/** Where a search shows others' words (its moments in the list, the moment's panel): a selection there never goes to
 *  Claude's view (`viewState`), since they may be another call's or another AI profile's (Intake and search R8). */
const SX_PRIVATE = '.sx-pm, .sx-moments, .sx-also, .sx-src'
/** Whether the page's selection at `el` (the element its end is in; any with `closest`) may be told to Claude: not in
 *  a terminal, not in a search's words. */
const sxSelectionShared = (el) => !(el && el.closest && (el.closest('.xterm') || el.closest(SX_PRIVATE)))
// end of the search request block

/** The window's search: on (search mode: the box's chips), its filters, the last answer, what is open. */
S.sx = { on: false, f: sxNoFilters(), q: '', res: null, busy: false, toggled: {}, more: false, sel: null, moment: null, menu: false, gen: 0 }
/** Intake and search are on (`/api/state`'s `brain`). */
const sxAvailable = () => !!(S.state && S.state.brain)
/** The list shows a search's results. */
const sxShown = () => S.sx.on && !!S.sx.res && S.view === 'calls'
const SX_ICON = (d, w = 14) => `<svg viewBox="0 0 24 24" width="${w}" height="${w}" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`
const SX_X = '<path d="M6 6l12 12M18 6L6 18"/>'
const SX_SEARCH = '<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>'
const sxKind = (id) => SX_KINDS.find((k) => k[0] === id) || SX_KINDS[3]
/** A day as a row says it: Today, Yesterday, a weekday this week, else "Nov 14" (with the year when not this one). */
const sxDay = (d) => {
  d = typeof d === 'string' ? new Date(`${d.slice(0, 10)}T12:00:00`) : d
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const that = new Date(d); that.setHours(0, 0, 0, 0)
  const diff = Math.round((today - that) / 86400000)
  if (diff === 0) return 'Today'
  if (diff === 1) return 'Yesterday'
  if (diff > 1 && diff < 7) return that.toLocaleDateString([], { weekday: 'short' })
  return that.toLocaleDateString([], { day: 'numeric', month: 'short', year: today.getFullYear() === that.getFullYear() ? undefined : 'numeric' })
}
const sxShort = (iso) => new Date(`${iso}T12:00:00`).toLocaleDateString([], { day: 'numeric', month: 'short' })
const sxToday = () => { const d = new Date(); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` }
/** A client's or a thread's name (its id when Echo doesn't know it). */
const sxName = (kind, id) => (kind === 'client' ? (projClient(id) || {}).name : (projById(id) || {}).name) || id

/** The box in search mode: the query with its clear button, then the kinds, the active filters and the filter menu. */
function sxHead() {
  const f = S.sx.f, res = S.sx.res
  const seg = `<span class="md-segs" role="group" aria-label="Sources">${SX_KINDS.map(([id, name, , d]) => `<button class="md-seg${f.kinds.includes(id) ? ' on' : ''}" data-sx-kind="${id}" aria-pressed="${f.kinds.includes(id)}" title="${esc(name)}" aria-label="${esc(name)}">${SX_ICON(d)}</button>`).join('')}</span>`
  const chips = sxChips(f, res, sxName, sxShort, sxToday()).map((c) => `<button class="sx-chip${c.guess ? ' guess' : ''}" data-sx-drop="${esc(c.key)}"${c.title ? ` title="${esc(c.title)}"` : ''} aria-label="${esc(`${c.label}: remove`)}">${esc(c.label)}<span class="x" aria-hidden="true">${SX_ICON(SX_X)}</span></button>`)
  const add = `<span class="sx-add-wrap"><button class="sx-chip add" data-sx-menu aria-haspopup="menu" aria-expanded="${S.sx.menu}" title="Filter by section, client, thread, person or date" aria-label="Filters">${SX_ICON('<path d="M4 5h16l-6 7.5V19l-4 1.5v-8z"/>')}</button>${S.sx.menu ? sxMenu() : ''}</span>`
  const count = res ? [[res.titles.length, 'call'], [res.sources.length, 'source']].filter(([k]) => k).map(([k, w]) => `${k} ${w}${k === 1 ? '' : 's'}`).join(' · ') : ''
  const n = count && !chips.length ? `<span class="sx-count">${count}</span>` : ''
  // Always while searching: an empty box with a filter set leaves the search too.
  const clear = `<button class="icon-btn" data-sx-clear title="Clear the search (esc)" aria-label="Clear the search">${SX_ICON(SX_X)}</button>`
  return `<div class="sx-q"><input class="search" id="q" placeholder="Search what was said" value="${esc(S.query)}" aria-label="Search calls">${clear}</div>
    <div class="sx-chips">${seg}${chips.join('')}${add}${n}</div>`
}

/** The filter menu: sections, clients, threads (each a pick), who said it, and days. */
function sxMenu() {
  const f = S.sx.f
  const item = (attrs, label, on) => `<button class="menu-item" role="menuitemcheckbox" aria-checked="${!!on}" ${attrs}>${on ? '✓ ' : ''}${esc(label)}</button>`
  const clients = ((S.pj.data && S.pj.data.clients) || []).filter((c) => !c.internal)
  const threads = ((S.pj.data && S.pj.data.projects) || []).filter((p) => p.group !== 'closed')
  const days = [['Past 7 days', 7], ['Past 30 days', 30], ['Past 90 days', 90]]
  const since = (n) => { const d = new Date(); d.setDate(d.getDate() - n); return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}` }
  return `<div class="menu sx-menu" role="menu">
    <div class="menu-label">Section</div>${SX_SECTIONS.map(([id, label]) => item(`data-sx-section="${id}"`, label, f.sections.includes(id))).join('')}
    ${clients.length ? `<div class="menu-sep" role="separator"></div><div class="menu-label">Client</div>${clients.map((c) => item(`data-sx-client="${esc(c.id)}"`, c.name, f.client === c.id)).join('')}` : ''}
    ${threads.length ? `<div class="menu-sep" role="separator"></div><div class="menu-label">Thread</div>${threads.map((p) => item(`data-sx-thread="${esc(p.id)}"`, p.name, f.thread === p.id)).join('')}` : ''}
    <div class="menu-sep" role="separator"></div><div class="menu-label">Person</div>
    <input class="search sx-person" data-sx-person placeholder="Who said it, then ↵" value="${esc(f.person || '')}" aria-label="Who said it">
    <div class="menu-sep" role="separator"></div><div class="menu-label">Date</div>${days.map(([label, n]) => item(`data-sx-since="${since(n)}"`, label, f.since === since(n) && !f.until)).join('')}
  </div>`
}

/** The results: titles and people, then what was said with its notes (no match, older than the index keeps). */
function sxBody() {
  const res = S.sx.res, f = S.sx.f
  if (res.error) return `<p class="error" style="padding:12px">The search failed: ${esc(res.error)}</p>`
  let out = ''
  if (res.titles.length) out += `<div class="group">Titles and people · ${res.titles.length}</div>${res.titles.map((c) => callRow(c, sxDay)).join('')}`
  out += `<div class="group">${esc(sxSaidLabel(f.kinds))}${res.sources.length ? ` · ${res.sources.length}` : ''}</div>`
  const open = sxOpen(res.sources, S.sx.toggled)
  const shown = S.sx.more ? res.sources : res.sources.slice(0, 6)
  for (const s of shown) {
    const k = sxKind(s.kind)
    const at = s.kind === 'call' && /^\d{4}-\d\d-\d\d_\d{6}/.test(s.call || '') ? new Date(`${s.call.slice(0, 10)}T${s.call.slice(11, 13)}:${s.call.slice(13, 15)}:00`) : null
    const when = at ? `${sxDay(at)} ${hm(at)}` : s.date ? sxDay(s.date) : ''
    const where = s.client ? sxName('client', s.client) : (s.threads[0] || {}).name || ''
    const n = s.moments.length
    const meta = [when, where, `${n} moment${n === 1 ? '' : 's'}`].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
    const isOpen = open.has(s.source)
    out += `<button class="row sx-src" role="option" data-sx-src="${esc(s.source)}" aria-expanded="${isOpen}" aria-selected="false">
      <span class="si" title="${esc(k[1])}" aria-label="${esc(k[1])}">${SX_ICON(k[3])}</span><span class="t">${esc(s.title)}</span><span class="sx-chev">${SX_ICON('<path d="M9 6l6 6-6 6"/>')}</span>
      <span class="m">${meta}</span></button>`
    if (isOpen) out += `<div class="sx-moments" role="group" aria-label="${esc(s.title)}">${s.moments.map(sxMoment).join('')}</div>`
  }
  if (!S.sx.more && res.sources.length > shown.length) { const m = res.sources.length - shown.length; out += `<button class="link-btn sx-more" data-sx-more>${m} more source${m === 1 ? '' : 's'}</button>` }
  if (!res.sources.length && !res.older) {
    const no = sxNothing(f, res.query, sxName)
    out += `<div class="sx-note">${SX_ICON(SX_SEARCH)}<span>${esc(no.text)}${no.wider ? ' <button class="link-btn" data-sx-wider>Search every source</button>' : ''}</span></div>`
  }
  if (res.listed && res.words.length && res.sources.length) out += `<div class="sx-note">${SX_ICON(SX_SEARCH)}<span>No section of those days has these words: their sources, newest first.</span></div>`
  if (res.older) out += sxOlder()
  return out
}

/** "Older than the index keeps": how long each kind stays, from today. */
function sxOlder() {
  const ago = (n) => { const d = new Date(); d.setDate(d.getDate() - n); return d.toLocaleDateString([], { day: 'numeric', month: 'short' }) }
  return `<div class="sx-note">${SX_ICON('<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>')}<span><b>Older than the index keeps.</b> Transcripts stay 45 days (since ${esc(ago(45))}), minutes and mail 90 (since ${esc(ago(90))}). Call cards stay, so titles and people still match.</span></div>`
}

/** A moment: its section, who said it and when, two lines of what was said with the words marked. */
function sxMoment(m) {
  const who = m.who ? `<span>${esc(m.who)}</span>` : ''
  return `<button class="sx-mo" role="option" data-sx-mo="${esc(m.chunk)}" aria-selected="${S.sx.sel === m.chunk}"><span class="s"><b>${esc(sxSection(m))}</b>${who}<span>${esc(sxWhen(m, clock, sxDay))}</span></span><span class="x">${sxMark(m.text, S.sx.res && S.sx.res.words, esc)}</span></button>`
}

/** Run the search for the box's words and the filters (a newer search drops an older answer). */
async function sxRun() {
  const q = S.query.trim()
  const f = S.sx.f
  if (!q && !f.since && !f.until) { S.sx.res = null; return render() }
  const gen = ++S.sx.gen
  S.sx.busy = true
  let res
  try { res = await get(`api/search?${sxRequest(f, q)}`) } catch (e) { res = { enabled: true, error: String(e) } }
  if (gen !== S.sx.gen) return
  S.sx.busy = false
  // Intake went off meanwhile: the box keeps to titles and people.
  if (!res.enabled) { S.sx.on = false; S.sx.res = null; return render() }
  // A new search: the panel never keeps a moment of an older one.
  sxClosePanel()
  S.sx.moment = null; S.sx.sel = null; S.sx.pmenu = false
  S.sx.res = { titles: [], sources: [], words: [], ...res }
  S.sx.toggled = {}; S.sx.more = false
  render()
  const rows = $('.rows'); if (rows) rows.scrollTop = 0
}

/** Search mode on (the box's chips), from Enter or ⌘K; `q` (when given) is the box's words. */
function sxStart(q) {
  if (!sxAvailable()) return
  S.sx.on = true
  if (q != null) S.query = q
  if (S.view !== 'calls') goActivity('calls')
  render()
  setTimeout(() => { const i = $('#q'); if (i) { i.focus(); i.setSelectionRange(i.value.length, i.value.length) } }, 0)
  if (S.query.trim()) sxRun()
}

/** The right panel closed when it shows a moment (its view goes with the search's moment). */
function sxClosePanel() {
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay)
  if (eff.right.shown && eff.right.active === 'said') { Layout.close('right'); layoutChanged() }
}

/** Back to the calls: the box empty, no filters, no results; the moment's panel view goes with them. */
function sxLeave() {
  sxClosePanel()
  S.sx = { ...S.sx, on: false, f: sxNoFilters(), res: null, toggled: {}, more: false, sel: null, moment: null, menu: false, gen: S.sx.gen + 1 }
  S.query = ''
  loadCalls(true).then(render)
  render()
}

/** A moment opened in the right panel ("What was said"), with the turns around it. */
async function sxOpenMoment(chunk) {
  S.sx.sel = chunk
  render()
  let m
  try { m = await get(`api/search/moment?chunk=${encodeURIComponent(chunk)}&q=${encodeURIComponent(S.sx.res ? S.sx.res.words.join(' ') : '')}`) } catch (e) { m = { error: String(e) } }
  if (S.sx.sel !== chunk) return
  S.sx.moment = m
  Layout.show('said', panelViews())
  layoutChanged()
}

/** The moment in the right panel: its source (a call's when, length and people), client and threads; the turns around
 *  it with the one said marked, or its message's facts; Open at, copy link, Ask Claude, more; and the source's other
 *  moments of this search. */
function sxPanel() {
  const m = S.sx.moment
  if (!m) return ''
  if (m.error) return `<div class="sx-pm"><p class="error">${esc(m.error)}</p></div>`
  const words = S.sx.res ? S.sx.res.words : []
  const mo = m.moment, k = sxKind(m.kind), c = m.call
  const d = c && c.started_at ? new Date(c.started_at) : null
  const facts = c ? [d ? `${sxDay(d)}, ${hm(d)}` : '', c.minutes ? `${c.minutes} min` : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('') + (c.people && c.people.length ? `<span class="people">${esc(c.people.join(', '))}</span>` : '')
    : [mo.day ? sxDay(mo.day) : '', mo.who || ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
  const cl = m.client && (projClient(m.client) || { id: m.client, name: m.client })
  const chips = (cl ? `<span class="pj-chip">${clientIcon(cl, 14)}${esc(cl.name)}</span>` : '') + (m.threads || []).map((t) => { const p = projById(t.id) || t; return `<span class="pj-chip">${projIcon(p, 14)}${esc(p.name || t.id)}</span>` }).join('')
  const turn = (t) => `<div class="turn ${t.me ? 'me' : 'remote'}${!t.me && t.spk ? ` spk${((t.spk - 1) % 6) + 1}` : ''}${t.hit ? ' hit' : ' ctx'}"><time>${esc(clock(Math.floor(t.t)))}</time><div><div class="who">${esc(t.who)}</div><p>${t.hit ? sxMark(t.text, words, esc) : esc(t.text)}</p></div></div>`
  const turns = (m.turns || []).length ? `<div class="transcript">${m.turns.map(turn).join('')}</div>` : ''
  let said
  if (mo.section_kind === 'transcript' || mo.section_kind === 'chat') said = `<div class="sx-sec">${esc(sxSection(mo))} <span>${esc(sxWhen(mo, clock, sxDay))}</span></div>${turns || `<p class="sx-said">${sxMark(mo.text, words, esc)}</p>`}`
  else if (c) said = `<div class="sx-sec">${esc(sxSection(mo))} <span>minutes</span></div><p class="sx-said">${sxMark(mo.text, words, esc)}</p>${turns ? `<div class="sx-sec">Said at <span>${esc(clock(mo.t))}</span></div>${turns}` : ''}`
  else said = `<div class="sx-sec">${esc(sxSection(mo))} <span>${esc(sxWhen(mo, clock, sxDay))}</span></div>${(m.lines || [mo.text]).map((l) => `<p class="sx-said">${sxMark(l, words, esc)}</p>`).join('')}`
  const go = mo.t != null && c ? `Open at ${clock(mo.t)}` : 'Open'
  const acts = `<div class="sx-acts"><button class="btn primary" data-sx-go>${esc(go)}</button>
    <button class="icon-btn" data-sx-copy title="Copy the link to this moment (echo://)" aria-label="Copy link">${SX_ICON('<path d="M10 14a4 4 0 0 0 5.66 0l3-3a4 4 0 0 0-5.66-5.66l-1 1"/><path d="M14 10a4 4 0 0 0-5.66 0l-3 3a4 4 0 0 0 5.66 5.66l1-1"/>')}</button>
    <button class="icon-btn" data-sx-claude title="Ask Claude about this" aria-label="Ask Claude"><span class="sb-claude" style="margin:0">${ICON(CLAUDE_MARK)}</span></button>
    <span class="sx-more-wrap"><button class="icon-btn" data-sx-pmenu aria-haspopup="menu" aria-expanded="${!!S.sx.pmenu}" title="More" aria-label="More">${SX_ICON('<circle cx="5" cy="12" r="1.2"/><circle cx="12" cy="12" r="1.2"/><circle cx="19" cy="12" r="1.2"/>')}</button>${S.sx.pmenu ? `<div class="menu" role="menu"><button class="menu-item" role="menuitem" data-sx-copytext>Copy what was said</button>${m.client ? `<button class="menu-item" role="menuitem" data-sx-only="${esc(m.client)}">Search only ${esc(sxName('client', m.client))}</button>` : ''}</div>` : ''}</span></div>`
  const src = S.sx.res && S.sx.res.sources.find((s) => s.source === m.source)
  const others = src ? src.moments.filter((x) => x.chunk !== mo.chunk) : []
  const also = others.length ? `<div class="sx-also"><div class="sx-sec">Also in this ${m.kind === 'call' ? 'call' : m.kind === 'document' ? 'document' : m.kind === 'mail' ? 'mail' : 'channel'}</div>${others.map(sxMoment).join('')}</div>` : ''
  return `<div class="sx-pm selectable"><div><h3>${SX_ICON(k[3])}${esc(c ? c.title : m.title)}</h3><div class="facts">${facts}</div>${chips ? `<div class="pj-chips">${chips}</div>` : ''}</div>${said}${acts}${also}</div>`
}
PANEL_VIEWS.unshift({ id: 'said', label: 'What was said', icon: ICON(SX_SEARCH), area: 'right', when: () => S.view === 'calls' && !!S.sx.moment, render: sxPanel })

/** Ask Claude about `call` once it is the open call with its profile known (`S.call.live`), so Claude runs on that
 *  call's own profile; not open within 10 s (another click took over): nothing. */
async function sxAskWhenOpen(call) {
  for (let i = 0; i < 50; i++) {
    if (S.view === 'calls' && S.call && S.call.name === call && S.call.live) return askClaudeAboutCall()
    if (!location.hash.startsWith(`#call/${call}`)) return
    await new Promise((ok) => setTimeout(ok, 200))
  }
}

/** Leaving Calls leaves the search: back there, the list is the calls. */
addEventListener('hashchange', () => setTimeout(() => { if (S.sx.on && S.view !== 'calls') sxLeave() }))

/** A filter changed: search again (when there is something to search). */
function sxFilter(change) {
  change(S.sx.f)
  if (S.sx.res || S.query.trim()) return sxRun()
  render()
}

document.addEventListener('input', (e) => {
  // New words after a search: the list goes back to the calls they match until Enter searches again.
  if (e.target.id === 'q' && S.sx.res) { S.sx.res = null; S.sx.gen++ }
})
document.addEventListener('keydown', (e) => {
  if (e.target.id === 'q' && e.key === 'Enter' && !e.isComposing && sxAvailable()) { e.preventDefault(); clearTimeout(S.qT); S.sx.on = true; return sxRun() }
  if (e.target.id === 'q' && e.key === 'Escape' && S.sx.on) { e.preventDefault(); e.stopPropagation(); return sxLeave() }
  if (e.target.matches && e.target.matches('[data-sx-person]') && e.key === 'Enter') { e.preventDefault(); const v = e.target.value.trim(); S.sx.menu = false; return sxFilter((f) => { f.person = v || null }) }
  if (e.key === 'Escape' && (S.sx.menu || S.sx.pmenu)) { S.sx.menu = false; S.sx.pmenu = false; return render() }
}, true)
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-sx-kind],[data-sx-drop],[data-sx-menu],[data-sx-section],[data-sx-client],[data-sx-thread],[data-sx-since],[data-sx-src],[data-sx-mo],[data-sx-more],[data-sx-wider],[data-sx-clear],[data-sx-go],[data-sx-copy],[data-sx-copytext],[data-sx-claude],[data-sx-pmenu],[data-sx-only],[data-sx-person]')
  // A click outside an open filter menu closes it.
  if (S.sx.menu && !(e.target.closest && e.target.closest('.sx-add-wrap'))) { S.sx.menu = false; if (!t) render() }
  if (S.sx.pmenu && !(e.target.closest && e.target.closest('.sx-more-wrap'))) { S.sx.pmenu = false; if (!t) render() }
  if (!t) return
  const d = t.dataset, m = S.sx.moment
  if (d.sxPerson !== undefined) return
  if (d.sxKind) return sxFilter((f) => { f.kinds = f.kinds.includes(d.sxKind) ? f.kinds.filter((k) => k !== d.sxKind) : [...f.kinds, d.sxKind] })
  if (d.sxDrop) {
    const key = d.sxDrop
    if (key === 'guess') { S.query = sxDropWords(S.query, S.sx.res && S.sx.res.filters.dates_from_query); return sxRun() }
    return sxFilter((f) => {
      if (key.startsWith('section:')) f.sections = f.sections.filter((s) => s !== key.slice(8))
      else if (key === 'days') { f.since = null; f.until = null } else f[key] = null
    })
  }
  if (d.sxMenu !== undefined) { S.sx.menu = !S.sx.menu; return render() }
  if (d.sxSection) return sxFilter((f) => { f.sections = f.sections.includes(d.sxSection) ? f.sections.filter((s) => s !== d.sxSection) : [...f.sections, d.sxSection] })
  if (d.sxClient) { S.sx.menu = false; return sxFilter((f) => { f.client = f.client === d.sxClient ? null : d.sxClient }) }
  if (d.sxThread) { S.sx.menu = false; return sxFilter((f) => { f.thread = f.thread === d.sxThread ? null : d.sxThread }) }
  if (d.sxSince) { S.sx.menu = false; return sxFilter((f) => { f.since = f.since === d.sxSince && !f.until ? null : d.sxSince; f.until = null }) }
  if (d.sxSrc) { const open = sxOpen(S.sx.res.sources, S.sx.toggled); S.sx.toggled[d.sxSrc] = !open.has(d.sxSrc); return render() }
  if (d.sxMo) return sxOpenMoment(d.sxMo)
  if (d.sxMore !== undefined) { S.sx.more = true; return render() }
  if (d.sxWider !== undefined) { S.sx.f = sxNoFilters(); return sxRun() }
  if (d.sxClear !== undefined) return sxLeave()
  if (d.sxOnly) { S.sx.pmenu = false; return sxFilter((f) => { f.client = d.sxOnly; f.thread = null }) }
  if (!m || !m.moment) return
  if (d.sxGo !== undefined) return m.moment.link ? window.echo.openLink(m.moment.link, true) : null
  if (d.sxCopy !== undefined) { send({ cmd: 'copy', text: m.moment.link }); t.title = 'Copied'; return }
  if (d.sxCopytext !== undefined) { S.sx.pmenu = false; send({ cmd: 'copy', text: m.moment.text }); return render() }
  if (d.sxPmenu !== undefined) { S.sx.pmenu = !S.sx.pmenu; return render() }
  // Ask Claude: a call's moment opens that call there, and Claude on the call's own profile (a call's text goes only to
  // its profile); another source's, the Claude panel.
  if (d.sxClaude !== undefined) {
    if (m.call && m.moment.link) return window.echo.openLink(m.moment.link, true).then(() => sxAskWhenOpen(m.call.name))
    return claudePanel('claude-folder')
  }
})
