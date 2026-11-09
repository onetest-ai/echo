// ── language of the next recording (Record ▾) ──────────────────────────────────────────────
// A call's language is chosen, never detected: Settings' first language (the default, as for the calls Echo starts by
// itself) unless another is picked here, until the window reloads.
function recordLang() {
  const configured = (S.state && S.state.languages) || []
  return S.recLang && configured.includes(S.recLang) ? S.recLang : (configured[0] || 'en')
}
function setRecordLang(v) { S.recLang = v }
const LANG_NAMES = (() => { try { return new Intl.DisplayNames(['en'], { type: 'language' }) } catch { return null } })()
const langName = (c) => { try { return (LANG_NAMES && LANG_NAMES.of(c)) || c.toUpperCase() } catch { return c.toUpperCase() } }
/** The languages on offer (Settings', the default first) and the current choice. */
function langChoices() {
  const configured = (S.state && S.state.languages) || []
  return { opts: configured.length ? configured : ['en'], cur: recordLang() }
}
// ── AI profiles: which model (and account) writes a call's live notes and minutes ──────────
const ai = () => (S.state && S.state.ai && S.state.ai.profiles) ? S.state.ai : { profiles: [], default: null, live_notes: false, error: S.state && S.state.ai && S.state.ai.error }
const profileOf = (id) => ai().profiles.find((p) => p.id === id)
const profileName = (id) => { const p = profileOf(id); return p ? p.name : (id || '') }
/** Where a profile's requests go, in words. */
const profileWhere = (p) => !p ? '' : p.kind === 'local' ? (p.host ? `at ${p.host}` : 'on this computer') : p.kind === 'claude' ? `with your Claude account${p.account ? ` (${p.account})` : ''}` : 'as Claude Code is set up'
/** A profile in a menu: a Claude one with the account its folder is signed in to ("— not signed
 *  in"; the recorded one until checked). */
function profileLabel(p) {
  if (!p || p.kind !== 'claude') return p ? p.name : ''
  const s = p.signin || {}
  if (s.state === 'signed_in' || s.state === 'other_account') return `${p.name} — ${s.account}`
  if (s.state === 'signed_out') return `${p.name} — not signed in`
  return p.account ? `${p.name} — ${p.account}` : p.name
}
function profileOptions(cur) {
  return ai().profiles.map((p) => `<option value="${esc(p.id)}"${p.id === cur ? ' selected' : ''}>${esc(profileLabel(p))}</option>`).join('')
}

/** A Claude profile's sign-in and what can be done about it (`signin.rs` → `p.signin`); '' when
 *  there is nothing to say: signed in (not just now), or not known yet and no error. */
function signinNote(p) {
  const s = p && p.kind === 'claude' && p.signin
  if (!s) return ''
  const id = esc(p.id), name = esc(p.name)
  const btn = (attrs, label, cls = 'btn') => `<button class="${cls}" ${attrs}>${esc(label)}</button>`
  const signIn = (label = 'Sign in…') => btn(`data-signin="${id}"`, label, 'btn primary')
  const note = (text, buttons, kind = '', more = '') => `<div class="signin ${kind}"><span class="signin-text">${text}</span>${buttons ? `<span class="signin-acts">${buttons}</span>` : ''}${more ? `<span class="hint">${more}</span>` : ''}</div>`
  const cli = `echoctl ai login ${p.id}`
  switch (s.state) {
    case 'signed_out': return note(`${name} isn't signed in.`, signIn())
    case 'starting': return note(`Starting the sign-in of ${name}…`, '', 'busy')
    case 'checking': return note('Checking the sign-in…', '', 'busy')
    case 'waiting': return note(`Sign in to ${name} in your browser${s.email ? ` (as ${esc(s.email)})` : ''}…`, btn(`data-signin-cancel="${id}"`, 'Cancel', 'btn ghost'), 'busy',
      `Browser didn't open? Cancel, then run <code>${esc(cli)}</code> in Terminal <button class="btn ghost" data-copy="${esc(cli)}">Copy</button>`)
    case 'stopping': return note('Stopping the sign-in…', '', 'busy')
    case 'elsewhere': return note(`${name} is being signed in in a terminal…`, '', 'busy')
    case 'failed': return note(`The sign-in of ${name} didn't finish: ${esc(s.error || '')}`, signIn('Try again'), 'error')
    case 'other_account': return note(`${name} is signed in as <b>${esc(s.account)}</b>, not ${esc(s.expected || 'the account it was set up with')}.`,
      btn(`data-signin-accept="${id}" data-account="${esc(s.account)}"`, `Use ${s.account}`) + signIn('Sign in again…'), 'error')
    case 'signed_in': return s.just ? note(`Signed in as ${esc(s.account)}.`, '', 'ok') : ''
    default: return s.error ? note(`${name}: ${esc(s.error)}`, btn(`data-signin-check="${id}"`, 'Check again', 'btn ghost') + signIn(), 'error') : ''
  }
}
/** A fold's chevron: right when closed, down when open (drawn, not a text glyph — "▸" was too faint to see). */
const CHEV = (open) => `<svg class="chev${open ? ' open' : ''}" viewBox="0 0 10 10" width="10" height="10" aria-hidden="true"><path d="M3.5 1.5L7 5l-3.5 3.5" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/></svg>`
const CARET = '<svg viewBox="0 0 10 6" width="8" height="5" aria-hidden="true"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>'

/** A record button with the language picker on its right (a native select over the caret). */
/** A memo's mark: a microphone (the list, the pill, the header). */
const MIC = (size = 13) => `<svg class="mic" viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><rect x="9" y="3" width="6" height="11" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v3"/></svg>`

function recordButton(attrs, label, primary = true, memo = false) {
  const { opts, cur } = langChoices()
  const cls = primary ? 'btn primary' : 'btn'
  const options = opts.map((c) => `<option value="${esc(c)}"${c === cur ? ' selected' : ''}>${esc(langName(c))}</option>`).join('')
  const title = `Transcribed in ${langName(cur)}: pick the call's language here`
  return `<span class="split"><button class="${cls} split-main" ${attrs}>${esc(label)}</button><label class="${cls} split-lang" title="${esc(title)}"><span>${esc(cur.toUpperCase())}</span>${CARET}<select data-reclang aria-label="Language of the call">${options}</select></label></span>${memo ? `<button class="pmemo" data-cmd="record-memo" title="Record a memo — just you, starts now; its threads are found from what you say" aria-label="Record a memo">${MIC(12)}Memo</button>` : ''}`
}

// ── views ──────────────────────────────────────────────────────────────────────────────────
const MAC = /Mac/.test(navigator.platform || navigator.userAgent)
const MOD = MAC ? '⌘' : 'Ctrl+'

/** A calendar meeting about to start (3 min before … 4 min after), not recorded or armed yet; not a Zoom or Google Meet
 *  one (Echo can't record it yet). */
function meetingSoon() {
  const now = Date.now()
  return (S.agenda || []).find((m) => {
    const s = new Date(m.start).getTime()
    return m.id && m.join_url && !otherApp(m) && now >= s - 3 * 60000 && now <= s + 4 * 60000
  }) || null
}

/** The recording's clock: `data-since` (ms), its text written only by `tickClocks` (after each render and every second),
 *  so the HTML around it stays the same and isn't drawn again as the seconds pass. */
const since = (iso) => { const t = iso ? new Date(iso).getTime() : NaN; return Number.isFinite(t) ? t : null }
function tickClocks() {
  for (const el of document.querySelectorAll('[data-since]')) {
    const t = Number(el.dataset.since)
    if (t) el.textContent = clock((Date.now() - t) / 1000)
  }
}

/** Record / Stop at the top centre of the window, in every state; the sign-in the default
 *  profile (the one a recording pins) needs, under it. */
function pill() {
  const st = S.state || {}
  if (st.recording) {
    const name0 = String(st.recording).split(/[\\/]/).pop()
    const c0 = (S.calls || []).find((x) => x.name === name0)
    const m0 = since(c0 && c0.started_at)
    // A memo: a mic where a call has its dot; Stop ends it at once (nothing else is recorded).
    if (st.recording_memo) {
      return `<div class="pill rec" role="group" aria-label="Recording a memo"><span class="pmic">${MIC(13)}</span>${m0 ? `<span class="clock" data-since="${m0}"></span>` : ''}<button class="ptitle link" data-call="${esc(name0)}" title="Open the memo">Memo</button><button class="pbtn stop" data-cmd="stop"><svg viewBox="0 0 10 10" width="9" height="9" aria-hidden="true"><rect width="10" height="10" rx="2" fill="currentColor"/></svg>Stop</button></div>`
    }
    if (S.stopAsk) {
      return `<div class="pill rec"><span class="ptitle">Stop recording?</span><button class="pbtn stop" data-cmd="stop">Stop</button><button class="pbtn ghost" data-cmd="stop-keep">Keep recording</button></div>`
    }
    const name = String(st.recording).split(/[\\/]/).pop()
    const c = (S.calls || []).find((x) => x.name === name)
    const t0 = since(c && c.started_at)
    return `<div class="pill rec" role="group" aria-label="Recording"><span class="dot" aria-hidden="true"></span>${t0 ? `<span class="clock" data-since="${t0}"></span>` : ''}<button class="ptitle link" data-call="${esc(name)}" title="Open this call">${esc(titleOf(st.recording))}</button><button class="pbtn stop" data-cmd="stop-ask"><svg viewBox="0 0 10 10" width="9" height="9" aria-hidden="true"><rect width="10" height="10" rx="2" fill="currentColor"/></svg>Stop</button></div>`
  }
  if (st.armed) {
    return `<div class="pill armed" role="group" aria-label="Armed"><span class="ring" aria-hidden="true"></span><span class="ptitle">Armed — records the next Teams call</span><button class="pbtn ghost" data-cmd="stop">Cancel</button></div>`
  }
  const soon = !st.in_call && meetingSoon()
  const main = soon
    ? `<div class="pill soon" role="group" aria-label="Meeting soon"><span class="clock">${esc(hm(new Date(soon.start)))}</span><span class="ptitle">${esc(soon.subject || 'Meeting')}</span><button class="pbtn" data-arm="${esc(soon.id)}">Record it</button></div>`
    : `<div class="pill idle">${recordButton('data-cmd="record"', st.in_call ? 'Record this call' : 'Record next call', true, true)}</div>`
  // A call being transcribed doesn't take the pill: Record stays one click away.
  const tr = (S.calls || []).find((c) => c.status === 'transcribing')
  const chip = tr ? `<button class="pchip" data-call="${esc(tr.name)}" title="${esc(tr.title)}">Writing the transcript…</button>` : ''
  const sign = signinNote(profileOf(ai().default))
  return main + chip + (sign ? `<div class="pill-note">${sign}</div>` : '')
}

const BAR_KEY = MAC ? '⌘\\' : 'Ctrl+\\'
const BAR_ICON = (open) => ICON(`<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/><path d="${open ? 'M15.5 10l-2 2 2 2' : 'M13.5 10l2 2-2 2'}"/>`)
/** The activity bar: folded to icons by default (each one's name and keys in a hint beside it), or open with the names
 *  and counts (⌘\ / Ctrl+\, the switch in the title strip after the traffic lights — `barButton` —, ⌘K; remembered in
 *  the window's layout). */
function barButton() {
  const open = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay, S.barOver).bar.open
  const what = open ? 'Fold the sidebar' : 'Expand the sidebar'
  return `<button class="icon-btn bar-flip" data-cmd="bar-flip" aria-expanded="${open}" aria-label="${what}" title="${what} (${esc(BAR_KEY)})">${BAR_ICON(open)}</button>`
}
function actbar() {
  const btn = (a, i) => {
    const found = a.id === 'projects' && S.pj.data ? S.pj.data.found_count || 0 : 0
    const badge = a.id === 'settings' && settingsWarnings() ? `<span class="act-badge" aria-label="${settingsWarnings()} to fix">${settingsWarnings()}</span>`
      : found ? `<span class="act-badge new" aria-label="${found} found for you">${found}</span>` : ''
    const key = a.id === 'settings' ? MOD + ',' : MOD + (i + 1)
    return `<button class="act" data-go="${esc(a.id)}"${S.view === a.id ? ' aria-current="page"' : ''} aria-label="${esc(a.label)}" data-hint="${esc(a.label)}" data-key="${esc(key)}">${a.icon}<span class="act-label">${esc(a.label)}</span>${badge}</button>`
  }
  const acts = shownActivities()
  const top = acts.filter((a) => !a.bottom), bottom = acts.filter((a) => a.bottom)
  return `<nav class="acts" aria-label="Activities">${top.map(btn).join('')}<span class="acts-grow"></span>${bottom.map((a) => btn(a, acts.indexOf(a))).join('')}</nav>`
}

/** Windows with Settings → Layout → "Echo's own title bar": no system frame, the strip is the window's title bar (#76). */
// Only in the app's window (`window.ipc`): a browser tab of `echoctl window --serve` has no window to move or close.
const ownBar = () => !MAC && !!window.ipc && !!(S.state && S.state.own_title_bar)
/** Minimise, maximise / restore and close at the strip's right end, 46 × 40 as Windows 11's; close goes the way
 *  Alt+F4 does (`close_request`: the page saves its notes first). */
function captionButtons() {
  if (!ownBar()) return ''
  const max = S.maximized // the app says so as the window resizes (`window.echo.maximized`): restore then
  // The top edge resizes, its ends diagonally — not while maximised.
  const edges = max ? '' : '<div class="rz-top" data-resize-top="n"></div><div class="rz-top nw" data-resize-top="nw"></div><div class="rz-top ne" data-resize-top="ne"></div>'
  const svg = (d) => `<svg viewBox="0 0 10 10" width="10" height="10" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1">${d}</svg>`
  return edges + `<div class="win-caption" role="group" aria-label="Window">`
    + `<button class="win-cap" data-cmd="win-min" aria-label="Minimize" title="Minimize">${svg('<path d="M0 5.5h10"/>')}</button>`
    + `<button class="win-cap" data-cmd="win-max" aria-label="${max ? 'Restore' : 'Maximize'}" title="${max ? 'Restore' : 'Maximize'}">${svg(max ? '<rect x="0.5" y="2.5" width="7" height="7"/><path d="M2.5 2.5v-2h7v7h-2"/>' : '<rect x="0.5" y="0.5" width="9" height="9"/>')}</button>`
    + `<button class="win-cap win-close" data-cmd="win-close" aria-label="Close" title="Close">${svg('<path d="M0.5 0.5l9 9M9.5 0.5l-9 9"/>')}</button></div>`
}
/** The top bar's right end: the account chip, then the Inbox. */
function topEnd() { return `<div class="top-end">${accountChip()}${inboxButton()}</div>` }
/** Inbox: not an activity — it shows or hides its panel over whatever is open. */
function inboxButton() {
  const eff = Layout.effective(window.innerWidth, window.innerHeight, panelViews(), S.overlay, S.barOver)
  const n = nyOpen().length, urgent = nyOpen().some((i) => i.urgent)
  const shown = Layout.AREAS.some((a) => eff[a].shown && eff[a].active === 'needs')
  return `<button class="icon-btn tb-inbox${shown ? ' on' : ''}" data-panel-flip="needs" aria-pressed="${shown}" aria-label="Inbox" data-hint="Inbox" data-key="${esc(NY_HINT)}">${NY_ENTRY}${n ? `<span class="act-badge${urgent ? '' : ' new'}" aria-label="${n} open">${n}</span>` : ''}</button>`
}
/** The account Echo works in: the default AI profile. With several, a native select over the
 *  label (as the language picker) makes another the default, as Settings' Make default does;
 *  with one, a click opens AI & minutes. */
function accountChip() {
  const a = ai(), p = profileOf(a.default)
  if (!p) return ''
  const title = `AI: ${profileLabel(p)} — ${profileWhere(p)}`
  if (a.profiles.length < 2) return `<button class="acct" data-setsec="ai" title="${esc(title)}" aria-label="AI profile: ${esc(p.name)}"><span>${esc(p.name)}</span></button>`
  return `<label class="acct" title="${esc(title)}"><span>${esc(p.name)}</span>${CARET}<select data-ai-default aria-label="AI profile">${profileOptions(p.id)}</select></label>`
}

function statusbar() {
  const a = ai(), p = profileOf(a.default)
  const parts = []
  // The next automatic minutes, a click away (#42).
  const next = (S.calls || []).map((c) => [c, minutesAhead(c)]).filter((x) => x[1]).sort((x, y) => x[1] - y[1])[0]
  const soon = next ? `<button class="sb-btn sb-minutes" data-minutes-of="${esc(next[0].name)}" title="Minutes at ${esc(hm(next[1]))}">⏱ Minutes for “${esc(next[0].title)}” in ${minutesLeft(next[1])} min</button>` : ''
  if (p) parts.push(`<span title="${esc(profileWhere(p))}">AI: ${esc(p.name)}</span>`)
  if (a.live_notes) parts.push('<span>Live notes on</span>')
  // The terminal panel: a toggle here, where VS Code keeps it.
  parts.push(`<button class="sb-btn" data-pal-open title="Actions (${MAC ? '⌘K' : 'Ctrl+K'})">Actions ${MAC ? '⌘K' : 'Ctrl+K'}</button>`)
  const live = [...Terms.entries()].some(([k, e]) => k.startsWith('claude:') && e.id != null && !e.exited)
  parts.push(`<button class="sb-btn" data-cmd="claude-panel" title="Claude (${MOD}\`)">${live ? '● ' : ''}<span class="sb-claude">${ICON(CLAUDE_MARK)}</span>Claude</button>`)
  return `${soon}<span class="grow"></span>${parts.join('')}`
}

/** A panel (right or bottom) with its views as tabs; nothing when it has none (`Layout.effective`). */
function panel(area, eff) {
  const p = eff[area]
  if (!p.shown) return ''
  const other = area === 'right' ? 'bottom' : 'right'
  const v = PANEL_VIEWS.find((x) => x.id === p.active)
  // Each view by its icon (its name as the tooltip); one without an icon by its name.
  const tabs = p.views.map((id) => { const x = PANEL_VIEWS.find((y) => y.id === id); return `<button class="tab${x.icon ? ' icon-tab' : ''}" role="tab" data-panel-show="${esc(id)}" aria-selected="${id === p.active}" title="${esc(x.label)}" aria-label="${esc(x.label)}">${x.icon || esc(x.label)}${x.badge && x.badge() ? `<span class="tab-n">${esc(String(x.badge()))}</span>` : ''}</button>` }).join('')
  const moveIcon = other === 'right'
    ? ICON('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>')
    : ICON('<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 14h18"/>')
  return `<div class="panel-resize ${area}" data-resize="${area}" title="Drag to resize · double-click: default size" aria-hidden="true"></div><div class="panel-head"><div class="tabs" role="tablist">${tabs}</div><span class="panel-title">${v ? esc(v.label) : ''}</span>
    <button class="icon-btn" data-panel-move="${esc(p.active)}" data-to="${other}" title="Move to the ${other} panel" aria-label="Move to the ${other} panel">${moveIcon}</button>
    <button class="icon-btn" data-panel-toggle="${area}" title="Close the panel" aria-label="Close the panel">${ICON('<path d="M6 6l12 12M18 6L6 18"/>')}</button></div>
    <div class="panel-body">${v ? v.render() : ''}</div>`
}

/** Show Claude (the Echo-folder session); again: hide it. With `view`: show that one. */
function claudePanel(view) {
  const known = panelViews()
  const id = view || 'claude-folder'
  const a = Layout.area(id, known)
  if (!a) return
  const eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (!view && eff[a].shown && eff[a].active === id) Layout.toggle(a); else { if (eff.narrow && a === 'right') S.overlay = true; Layout.show(id, known) }
  layoutChanged()
}

let layoutTimer = null
/** The layout changed: draw it, and save it a moment later (one write for a burst of changes). */
/** The activity bar open or folded; in a narrow window it opens over the page for now (not saved). */
function flipBar() {
  if (window.innerWidth < 900) { S.barOver = !S.barOver; return render() }
  Layout.toggleBar()
  layoutChanged()
}

function layoutChanged() {
  clearTimeout(layoutTimer)
  layoutTimer = setTimeout(() => send({ cmd: 'save_layout', layout: Layout.toJSON() }), 500)
  render()
}

function titleOf(dir) {
  if (!dir) return ''
  const name = String(dir).split(/[\\/]/).pop()
  const c = (S.calls || []).find((x) => x.name === name)
  return c ? c.title : name
}

/** A call's status as a small icon left of its title (its name as the tooltip). */
const STATUS_ICON = {
  notes: '<path d="M6 3h8l4 4v14H6z"/><path d="M9 13.5l2 2 4-4.5"/>',
  transcript: '<path d="M5 7h14M5 11h14M5 15h9"/>',
  transcribing: '<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>',
  failed: '<path d="M12 4l9 16H3z"/><path d="M12 10v4M12 17v.5"/>',
  audio: '<path d="M4 10v4M8 7v10M12 4v16M16 8v8M20 11v2"/>',
}
function statusIcon(status) {
  const label = STATUS[status] || status || ''
  const inner = status === 'recording' ? '<span class="rec-dot"></span>'
    : `<svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${STATUS_ICON[status] || STATUS_ICON.transcript}</svg>`
  return `<span class="si ${esc(status || '')}" title="${esc(label)}" aria-label="${esc(label)}">${inner}</span>`
}

function list() {
  // The app searched and paged them already (`loadCalls`).
  const q = S.query.trim()
  const calls = S.calls || []
  let body
  if (S.calls === null) body = `<div class="skeleton" style="padding:12px">${'<i></i>'.repeat(8)}</div>`
  else if (S.error) body = `<p class="error" style="padding:12px">Can't read the calls: ${esc(S.error)}</p>`
  else if (!calls.length) body = q ? `<p style="padding:14px;color:var(--text-3)">Nothing matches "${esc(S.query)}".</p>` : ''
  else {
    let group = null
    body = calls.map((c) => {
      const d = c.started_at ? new Date(c.started_at) : null
      const g = d ? dayLabel(d) : 'Undated'
      const head = g !== group ? `<div class="group">${esc(g)}</div>` : ''
      group = g
      const meta = [d ? hm(d) : '', c.memo ? 'Memo' : '', c.imported ? 'Teams transcript' : '', c.minutes ? `${c.minutes} min` : '', c.people.length ? c.people.slice(0, 2).join(', ') + (c.people.length > 2 ? ` +${c.people.length - 2}` : '') : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
      const pjs = ((S.pj.data && S.pj.data.by_call) || {})[c.name] || []
      const icons = pjs.length ? `<span class="pj-icons">${pjs.slice(0, 3).map((p) => projIcon(p, 14)).join('')}</span>` : ''
      const due = minutesAhead(c)
      const clockChip = due ? `<span class="mn-clock" title="Minutes at ${esc(hm(due))}">⏱ ${minutesLeft(due)} min</span>` : ''
      return `${head}<button class="row" role="option" data-call="${esc(c.name)}" aria-selected="${c.name === S.selected}">
        ${c.memo && c.status !== 'recording' ? `<span class="si memo" title="Memo" aria-label="Memo">${MIC(14)}</span>` : statusIcon(c.status)}
        <span class="t">${esc(c.title)}</span>
        <span class="m">${clockChip}${icons}${meta}</span>
      </button>`
    }).join('') + (S.callsNext ? `<div class="rows-more" data-calls-more>${S.callsMore ? 'Loading…' : `${calls.length} of ${S.callsTotal}`}</div>` : '')
  }
  // Today's meetings still to come, each a call-to-be with its prep.
  const ups = !q && S.up && S.up.meetings ? S.up.meetings : []
  // Folded by default (a full day is a long list): a head like a call's row — how many, and the next one.
  // One going on now first ("Now"), else the next to start.
  const nowMs = Date.now(), nx = ups.find((u) => u.start && new Date(u.start) <= nowMs) || ups[0]
  const going = nx && nx.start && new Date(nx.start) <= nowMs, nxAt = nx && nx.start ? hm(new Date(nx.start)) : ''
  const upHead = `<button class="row up-fold" data-up-fold aria-expanded="${!!S.upOpen}" title="${S.upOpen ? 'Fold' : 'Show them'}">
      <span class="si up" aria-hidden="true">${ICON('<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>').replace('width="20" height="20"', 'width="14" height="14"')}</span>
      <span class="t">Coming up today<span class="up-n">${ups.length}</span></span>
      <span class="m">${nx ? `<span>${going ? 'Now' : `Next ${esc(nxAt)}`}</span><span>${esc(nx.subject || 'A meeting')}</span>` : ''}</span>
      <span class="up-chev" aria-hidden="true">${ICON('<path d="M9 6l6 6-6 6"/>')}</span>
    </button>`
  const upRows = ups.length ? upHead + (S.upOpen ? ups : []).map((u) => {
    // As a call's row: only its icon differs (a clock: still to come).
    const st = u.start ? new Date(u.start) : null, en = u.end ? new Date(u.end) : null
    const mins = st && en ? Math.round((en - st) / 60000) : 0
    const names = u.names || []
    const meta = [st ? hm(st) : '', mins ? `${mins} min` : '', names.length ? names.slice(0, 2).join(', ') + (names.length > 2 ? ` +${names.length - 2}` : '') : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
    const pjs = u.projects || []
    const icons = pjs.length ? `<span class="pj-icons">${pjs.slice(0, 3).map((p) => projIcon(p, 14)).join('')}</span>` : ''
    return `<button class="row" role="option" data-up="${esc(u.id)}" aria-selected="${S.upSel === u.id}" title="${esc(u.writing ? 'Writing the prep…' : u.prep ? 'Prep ready' : 'No prep yet')}">
      <span class="si up" aria-label="Coming up">${ICON('<circle cx="12" cy="12" r="8"/><path d="M12 8v4l3 2"/>').replace('width="20" height="20"', 'width="14" height="14"')}</span>
      <span class="t">${esc(u.subject || 'A meeting')}</span>
      <span class="m">${icons}${meta}</span>
    </button>`
  }).join('') : ''
  if (upRows && body !== undefined) body = upRows + body
  return `<section class="list">
    <div class="list-head"><div class="list-title"><h1>Calls</h1><button class="icon-btn" data-panel-show="vocabulary" title="Vocabulary — names and terms transcription should hear right" aria-label="Vocabulary">${BOOK}</button></div><input class="search" id="q" placeholder="Search titles and people" value="${esc(S.query)}" aria-label="Search calls"></div>
    <div class="rows" role="listbox" aria-label="Calls">${body}</div>
  </section>`
}

/** In a narrow window the list folds away: this opens it over the call (`.list-open`). */
function listToggle() {
  const c = S.selected && (S.calls || []).find((x) => x.name === S.selected)
  const d = c && c.started_at ? new Date(c.started_at) : null
  return `<button class="btn ghost list-toggle" data-cmd="list-toggle" aria-expanded="${S.listOpen}">Calls${d ? ` · ${esc(dayLabel(d))}` : ''} ${CARET}</button>`
}

function emptyMain() {
  if (S.calls && !S.calls.length) {
    return `<div class="empty"><div>${mark()}<h3>No calls yet</h3><p>Use Record at the top of this window or the menu-bar octopus. Echo transcribes each call on this computer when it ends.</p></div></div>`
  }
  return `${listToggle()}<div class="empty"><div>${mark()}<h3>Pick a call</h3><p>Its minutes, your notes, the transcript, chat and shared screens open here.</p></div></div>`
}

/** The call view in regions: `plain` (nothing to show yet), else its header and its pane — on the
 *  Minutes tab with minutes, the pane's bar and its frame apart, so a refresh that changes the bar
 *  never reloads the page in the frame (`render`). */
function doc() {
  if (S.upSel) return upcomingDoc()
  if (!S.selected) return { plain: emptyMain() }
  const c = S.call
  if (!c) return { plain: `<div class="pane"><div class="skeleton">${'<i></i>'.repeat(10)}</div></div>` }
  if (c.error) return { plain: `<div class="empty"><div><h3>This call can't be opened</h3><p class="error">${esc(c.error)}</p></div></div>` }
  const d = c.started_at ? new Date(c.started_at) : null
  const segs = Array.isArray(c.segments) ? c.segments : []
  const chat = Array.isArray(c.chat) ? c.chat : []
  const frames = Array.isArray(c.frames) ? c.frames : []
  const tabs = callTabs({ recording: c.status === 'recording', chat: chat.length, frames: frames.length, prep: !!c.prep })
  // While a call records (or is transcribed), open it on its Transcript once; once its minutes are
  // written, on Minutes once. The user's own choice stays. (`live` was the Transcript's live twin.)
  if (S.tab === 'live') S.tab = 'transcript'
  if (S.autoTab !== c.name && (c.status === 'recording' || c.status === 'transcribing')) { S.autoTab = c.name; S.tab = 'transcript' }
  if (S.autoTab === c.name && S.tab === 'transcript' && c.notes && c.notes_run && c.notes_run.state === 'done') { S.autoTab = 'done:' + c.name; S.tab = 'minutes' }
  if (!tabs.some((t) => t[0] === S.tab)) S.tab = 'minutes'
  // A tab the window chose itself (above) is written into the URL too, or a click on the tab the URL still names
  // would ask for the URL it already has and change nothing (#148).
  const own = `#call/${c.name}/${S.tab}`
  if (S.selected === c.name && hashText() !== own) history.replaceState(null, '', own)
  const people = c.people && c.people.length ? `<span class="people" title="${esc(c.people.join(', '))}">${esc(c.people.slice(0, 6).join(', '))}${c.people.length > 6 ? ` and ${c.people.length - 6} more` : ''}</span>` : ''
  const head = `${listToggle()}<div class="head-row">
        <div style="min-width:0">
          <h2 class="selectable">${esc(c.title)}</h2>
          <div class="facts">${c.memo ? `<span class="memo-fact">${MIC(12)} Memo</span>` : ''}${d ? `<span>${esc(d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }))}, ${esc(hm(d))}</span>` : ''}${c.minutes ? `<span>${c.minutes} min</span>` : ''}${meetingTypeChip(c)}${c.imported ? '<span title="Made from the meeting\'s Teams transcript: no audio, no live notes">from Teams transcript</span>' : ''}${people}</div>
          ${callProjects(c)}
        </div>
        <div class="doc-actions">${callMenu(c)}</div>
      </div>
      ${callSuggestion(c)}
      <div class="tabs" role="tablist">${tabs.map(([id, label, n, rec]) => `<button class="tab" role="tab" data-tab="${id}" aria-selected="${S.tab === id}">${rec ? '<span class="tab-rec" aria-label="Recording"></span>' : ''}${label}${n !== '' ? `<span class="n">${n}</span>` : ''}</button>`).join('')}</div>`
  const body = tabBody(c, segs, chat, frames)
  const confirm = S.confirmDelete ? confirmBox(c) : ''
  return typeof body === 'string' ? { head, pane: confirm + body } : { head, bar: confirm + body.bar, frame: body.frame }
}

/** Today (the first activity; the window opens on it): cards delivered to Echo (a routine's morning brief — Got it
 *  hides one until its source sends the next), your day (today's meetings: the call, Show prep or Prepare, Record when
 *  it starts) and your work (Tasks' piles, a few of each). An empty part isn't drawn. */
