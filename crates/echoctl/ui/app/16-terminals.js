// ── terminals (`terminal.rs`): each session a pseudo-terminal in the app, shown with xterm.js ───
// The app owns the program (closing the window keeps it; quitting Echo ends it). Here each has
// an element made once and kept: render() draws a slot (`data-term`), and the element moves into
// it — out of sight, it waits in a hidden holder, so a re-render, a tab or a panel move never
// loses the scrollback. Output comes by a long poll of `api/pty/<id>?from=<byte>` (all of it after
// a window reopens: the app keeps 2 MB); keys and size go back over IPC.
const Terms = new Map() // key → {id, term, fit, el, from, exited, polling, cols, rows}

/** Light or dark as the user chose (Settings → General → Appearance; this computer's window), else the system's.
 *  The CSS already has both (`:root[data-theme]`); graphs and terminals are drawn again in the new colours. */
const Theme = (() => {
  const KEY = 'echo.theme'
  const read = () => { try { const v = localStorage.getItem(KEY); return v === 'light' || v === 'dark' ? v : '' } catch { return '' } }
  const apply = () => {
    const v = read()
    if (v) document.documentElement.dataset.theme = v; else delete document.documentElement.dataset.theme
    document.documentElement.style.colorScheme = v || ''
  }
  return {
    chosen: read,
    apply,
    set(v) {
      try { if (v) localStorage.setItem(KEY, v); else localStorage.removeItem(KEY) } catch { /* storage off */ }
      apply()
      GraphView.retheme()
      for (const e of Terms.values()) if (e.term) e.term.options.theme = termTheme()
      render()
    },
  }
})()
Theme.apply()
const termTheme = () => {
  const css = getComputedStyle(document.documentElement)
  const v = (n) => css.getPropertyValue(n).trim()
  return { background: v('--surface') || '#1b1b21', foreground: v('--text') || '#ececf1', cursor: v('--accent') || '#9b78ff', selectionBackground: v('--accent-soft') || 'rgba(155,120,255,.25)' }
}
function termHolder() {
  let h = $('#term-holder')
  if (!h) { h = document.createElement('div'); h.id = 'term-holder'; h.hidden = true; document.body.appendChild(h) }
  return h
}
/** The element of a terminal (made on first use, then kept). */
function termEntry(key) {
  let e = Terms.get(key)
  if (e) return e
  if (!window.Terminal) return null
  const el = document.createElement('div')
  el.className = 'term'
  const term = new window.Terminal({ fontFamily: getComputedStyle(document.documentElement).getPropertyValue('--mono') || 'Menlo, monospace', fontSize: 12.5, cursorBlink: true, allowProposedApi: false, theme: termTheme(), scrollback: 5000 })
  const fit = new window.FitAddon.FitAddon()
  term.loadAddon(fit)
  term.open(el)
  e = { id: null, term, fit, el, from: 0, exited: false, polling: false, cols: 0, rows: 0 }
  term.onData((d) => { if (e.id != null && !e.exited) send({ cmd: 'pty_input', id: e.id, data: d }) })
  term.attachCustomKeyEventHandler((k) => termKey(e, k))
  Terms.set(key, e)
  termHolder().appendChild(el)
  return e
}
/** Keys xterm.js would get wrong here (false = handled). macOS: the window has no Edit menu, so
 *  ⌘C / ⌘V / ⌘A never reach the webview's own copy and paste — copy the selection, paste through
 *  the app (it reads the clipboard; an image there goes on as Ctrl+V, which Claude Code reads).
 *  Ctrl+Enter and Shift+Enter: a new line in Claude Code's prompt (Option+Enter's ESC CR), not Enter. */
function termKey(e, k) {
  const live = e.id != null && !e.exited
  const cmd = MAC && k.metaKey && !k.ctrlKey && !k.altKey && !k.shiftKey ? k.key.toLowerCase() : ''
  if (cmd === 'c' || cmd === 'v' || cmd === 'a') {
    k.preventDefault()
    if (k.type !== 'keydown') return false
    if (cmd === 'c' && e.term.hasSelection()) send({ cmd: 'copy', text: e.term.getSelection() })
    if (cmd === 'v' && live) send({ cmd: 'pty_paste', id: e.id, bracketed: !!e.term.modes.bracketedPasteMode })
    if (cmd === 'a') e.term.selectAll()
    return false
  }
  if (k.key === 'Enter' && (k.ctrlKey || k.shiftKey) && !k.metaKey && !k.altKey) {
    k.preventDefault()
    if (k.type === 'keydown' && live) send({ cmd: 'pty_input', id: e.id, data: '\x1b\r' })
    return false
  }
  return true
}
/** Follow a terminal's output until its program ends. */
async function termPoll(e) {
  if (e.polling) return
  e.polling = true
  const bytes = (b64) => Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
  while (e.id != null) {
    const id = e.id
    let r
    try { r = await get(`api/pty/${id}?from=${e.from}`) } catch { if (e.id === id) { e.exited = true; render() } break }
    if (e.id !== id) break
    if (r.reset) e.term.reset()
    if (r.data) { e.term.write(bytes(r.data)); e.lastData = Date.now() }
    e.from = r.next
    if (r.exited && !r.data) {
      if (!e.exited) { e.exited = true; try { S.sessions = new Set(await get('api/sessions')) } catch { /* as it was */ } render() }
      break
    }
  }
  e.polling = false
}
/** Fit a shown terminal to its slot, and tell the app its size. */
function termFit(e) {
  if (!e.el.isConnected || e.el.closest('#term-holder')) return
  try { e.fit.fit() } catch { return }
  if (e.id != null && (e.term.cols !== e.cols || e.term.rows !== e.rows)) {
    e.cols = e.term.cols; e.rows = e.term.rows
    send({ cmd: 'pty_resize', id: e.id, cols: e.cols, rows: e.rows })
  }
}
/** After a render: each terminal in its slot, the others in the holder. */
function attachTerms(focusKey) {
  const shown = new Set()
  for (const slot of document.querySelectorAll('.term-slot[data-term]')) {
    const e = Terms.get(slot.dataset.term)
    if (!e) continue
    shown.add(slot.dataset.term)
    if (e.el.parentElement !== slot) slot.appendChild(e.el)
    termFit(e)
  }
  for (const [k, e] of Terms) if (!shown.has(k) && e.el.parentElement !== termHolder()) termHolder().appendChild(e.el)
  if (focusKey && shown.has(focusKey)) Terms.get(focusKey).term.focus()
}
const focusedTerm = () => { for (const [k, e] of Terms) if (e.el.contains(document.activeElement)) return k; return null }
/** Start a terminal of `kind` for `key` (the app answers with `ptyOpened`). */
function termStart(kind, key, extra = {}) {
  const e = termEntry(key)
  if (S.convAsk && S.convAsk.key === key) S.convAsk = null
  if (!e) { S.notice = "The terminal can't be shown (xterm.js didn't load)."; return render() }
  e.exited = false; e.starting = true; e.signin = null; e.error = null; e.from = 0; e.term.reset()
  if (S.termOutput) delete S.termOutput[key]
  const slot = document.querySelector(`.term-slot[data-term="${CSS.escape(key)}"]`)
  const dims = slot ? (() => { slot.appendChild(e.el); try { e.fit.fit() } catch {} return { cols: e.term.cols, rows: e.term.rows } })() : { cols: 100, rows: 30 }
  send({ cmd: 'pty_open', kind, key, ...extra, ...dims })
  render()
}
/** Terminals the app already runs (the window reopened): shown again, scrollback and all. */
async function loadTerms() {
  try { S.sessions = new Set(await get('api/sessions')) } catch { /* none known */ }
  let list = []
  try { list = await get('api/ptys') } catch { return }
  S.ptyStale = Object.fromEntries(list.filter((t) => t.stale).map((t) => [t.key, t.stale]))
  for (const t of list) {
    const e = termEntry(t.key)
    if (!e || e.id === t.id) continue
    e.id = t.id; e.from = 0; e.exited = t.exited
    termPoll(e)
  }
}
new ResizeObserver(() => { for (const e of Terms.values()) termFit(e) }).observe(document.documentElement)

/** What the app knows of its terminals beyond their output: what a Claude session hasn't loaded (#44). */
async function loadPtyInfo() {
  try { S.ptyStale = Object.fromEntries((await get('api/ptys')).filter((t) => t.stale).map((t) => [t.key, t.stale])) } catch { /* as it was */ }
}
/** A Claude panel's conversation controls (#44): Restart, New conversation, Conversations ▾ — and, when it runs
 *  older plugins or skills than installed, Restart to update. */
function claudeTools(key, kind, extra, running) {
  const data = `data-term-kind="${esc(kind)}" data-term-key="${esc(key)}"${extra.call ? ` data-term-call="${esc(extra.call)}"` : ''}`
  // Only when it matters: the session runs older plugins or skills than installed. Restart and a new conversation
  // are ⌘K actions (claudeSessionActions).
  const stale = running && S.ptyStale && S.ptyStale[key]
  const hint = stale ? `<div class="term-stale"><span class="dot" aria-hidden="true"></span><span class="grow">${esc(stale.what)}${stale.at ? ` at ${esc(hm(new Date(stale.at)))}` : ''} — this session still runs what it started with.</span><button class="btn primary sm" data-conv="restart" ${data}>Restart to update</button></div>` : ''
  // Asked first while it runs: what it is doing stops (the conversation itself is kept).
  const ask = S.convAsk && S.convAsk.key === key && running ? S.convAsk : null
  const askText = !ask ? '' : ask.session === '' ? 'Restart Claude? Whatever it is doing now stops, and an answer being written is lost. The conversation is kept and goes on after the restart.'
    : 'Start a new conversation? This session stops, and whatever it is doing now is lost.'
  const confirm = ask ? `<div class="term-stale term-ask" role="alert"><span class="grow">${esc(askText)}</span><button class="btn ghost sm" data-conv-keep>Keep it running</button><button class="btn primary sm" data-conv-go="${esc(ask.session)}" ${data}>${ask.session === '' ? 'Restart' : 'Start new'}</button></div>` : ''
  return confirm || hint
}
/** ⌘K: restart a Claude panel's session (plugins and settings read afresh) or start a new conversation there. */
function claudeSessionActions(add) {
  const panels = [['claude-folder', 'claude:folder', 'Claude', { profile: S.folderProfile || ai().default || undefined }]]
  for (const [kind, key, label, extra] of panels) {
    const e = Terms.get(key), running = !!(e && e.id != null && !e.exited)
    const act = (session) => () => {
      claudePanel(kind)
      // While it runs the panel asks first; else it starts at once.
      if (running) { S.convAsk = { key, session }; render() } else termRestart(kind, key, extra, session)
    }
    add('Claude', `Restart ${label}`, act(''), { keywords: 'claude session reload plugins update', disabled: running ? null : 'not running' })
    add('Claude', `New conversation in ${label}`, act('new'), { keywords: 'claude session fresh clear' })
  }
}
/** Restart a Claude panel's session, start a new conversation, or go back to one (`session`: '' | 'new' | an id). */
async function termRestart(kind, key, extra, session) {
  const e = Terms.get(key)
  if (e && e.starting) return // one at a time
  if (e && e.id != null && !e.exited) send({ cmd: 'pty_close', id: e.id })
  if (e) { e.id = null; e.exited = false; e.starting = true }
  if (S.ptyStale) delete S.ptyStale[key]
  render()
  // The old session's processes stop first (the same conversation must never run twice): up to 8 s.
  for (let i = 0; i < 40; i++) {
    await new Promise((ok) => setTimeout(ok, 200))
    let list = []
    try { list = await get('api/ptys') } catch { continue }
    if (!list.some((t) => t.key === key && t.stopping)) break
  }
  termStart(kind, key, { ...extra, session: session || undefined })
}

/** A terminal panel view: the slot, or what starts it. */
function termView(key, kind, startLabel, intro = '', extra = {}) {
  const e = Terms.get(key)
  const running = e && e.id != null && !e.exited
  const attrs = `data-term-start="${esc(kind)}" data-term-key="${esc(key)}"${extra.call ? ` data-term-call="${esc(extra.call)}"` : ''}`
  const head = key.startsWith('claude:') ? claudeTools(key, kind, extra, running) : ''
  return head + termBody(e, key, kind, startLabel, intro, extra, running, attrs)
}
function termBody(e, key, kind, startLabel, intro, extra, running, attrs) {
  // Ended (or couldn't start): a card like the one before starting — what happened, and Resume —
  // not an empty terminal. What it printed stays one click away (an error is worth reading).
  if (e && e.exited && !(S.termOutput || {})[key]) {
    const why = e.error ? `<p class="term-error">${esc(e.error)}</p>` : ''
    const msg = e.signin ? 'This profile needs a sign-in before Claude can start.' : e.error ? "Claude couldn't start." : 'The session ended. Resume picks up the conversation where it stopped.'
    const signin = e.signin ? `<button class="btn primary" data-signin="${esc(e.signin)}">Sign in…</button>` : ''
    return `<div class="term-empty"><p>${msg}</p>${why}<div class="term-acts">${signin}<button class="btn${e.signin ? '' : ' primary'}" ${attrs}>${e.error || e.signin ? 'Try again' : 'Resume'}</button>${e.error ? '' : `<button class="btn ghost" data-term-output="${esc(key)}">Show output</button>`}</div></div><div class="term-slot" data-term="${esc(key)}" hidden></div>`
  }
  if (!running && !(e && (e.exited || e.starting))) {
    return `<div class="term-empty">${intro}<button class="btn primary" ${attrs}>${esc(startLabel)}</button></div><div class="term-slot" data-term="${esc(key)}" hidden></div>`
  }
  const head = e && e.exited ? `<div class="term-bar"><span class="grow">The session ended — its output.</span><button class="btn ghost" data-term-output="${esc(key)}">Hide</button><button class="btn" ${attrs}>Resume</button></div>`
    : e && e.starting ? '<div class="term-bar"><span class="grow">Starting Claude Code…</span></div>' : ''
  return `<div class="term-wrap">${head}<div class="term-slot" data-term="${esc(key)}"></div></div>`
}
// ── Claude in the window (terminal.rs): one session, the user's own Claude Code in the data folder on the
// profile they pick. "Ask Claude about this call" opens it: the open call reaches it in `<echo_view>`. (A second,
// guarded session bound to one call was dropped, 2026-11-04: two Claudes confused people.)
/** "Ask Claude about this call": the Claude panel on the call's own profile — a session on another one isn't told
 *  about the call (`view::restrict`: a call's text stays with its profile). Not running yet: it starts on the call's
 *  profile; running on another: the panel says so. */
function askClaudeAboutCall() {
  const cp = S.call && S.call.live && S.call.live.profile
  const e = Terms.get('claude:folder'), running = !!(e && e.id != null && !e.exited)
  if (cp && profileOf(cp) && !running) S.folderProfile = cp === ai().default ? null : cp
  claudePanel('claude-folder')
}
function claudeFolderView() {
  const a = ai()
  const pick = a.profiles.length > 1 ? `<label class="term-profile">Profile <select data-term-profile>${profileOptions(S.folderProfile || a.default)}</select></label>` : ''
  const cp = S.view === 'calls' && S.call && S.call.live && S.call.live.profile
  const fp = S.folderProfile || a.default || 'system'
  const other = cp && cp !== fp && profileOf(cp) ? `<div class="term-stale"><span class="grow">The open call was recorded on ${esc(profileOf(cp).name)}; Claude here runs on ${esc((profileOf(fp) || {}).name || fp)}, so it isn't told about the call. Pick ${esc(profileOf(cp).name)} as its profile to ask about it.</span></div>` : ''
  const intro = `<p>Ask Claude about anything in Echo — the call or thread you have open (it sees what you are looking at), your clients, people, tasks. It is your own Claude Code, with your settings, working in Echo's data folder.</p>${pick}`
  const saved = S.sessions && S.sessions.has(`${S.folderProfile || a.default || 'system'}:folder`)
  return other + termView('claude:folder', 'claude-folder', saved ? 'Resume Claude' : 'Start Claude', intro, { profile: S.folderProfile || a.default || '' })
}
PANEL_VIEWS.push({ id: 'claude-folder', label: 'Claude', icon: ICON(CLAUDE_MARK), area: 'bottom', render: claudeFolderView })

