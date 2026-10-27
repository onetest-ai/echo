// Echo window. Vanilla JS, no dependencies. Data comes from echo:// (`api/*`), actions go to the
// app over `window.ipc.postMessage`. Everything shown is escaped first: chat and transcripts are
// written by other people.
'use strict'

const $ = (sel, root = document) => root.querySelector(sel)
const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c])
const send = (msg) => window.ipc && window.ipc.postMessage(JSON.stringify(msg))
const get = async (path) => {
  const r = await fetch(path)
  if (!r.ok) throw new Error(`${r.status} ${path}`)
  return r.json()
}

const S = {
  view: 'calls', // calls | calendar
  cal: { mode: 'week', offset: 0, data: null, selected: null, loading: false },
  calls: null,
  agenda: null,
  state: null,
  selected: null,
  call: null,
  tab: 'notes',
  query: '',
  find: '',
  jump: null, // seconds to scroll the transcript to
  confirmDelete: false,
  lightbox: null,
  error: null,
  liveDraft: '', // the question being typed in the Live tab (kept across refreshes)
  recordProfile: null, // the AI profile picked for the next recording (null = the default)
  notesProfile: null, // {call, id}: the AI profile picked for Write notes
  prepOpen: {}, // call → the "Last time" card's open state, once the user toggled it
  autoTab: null, // the call whose tab was last chosen for the user (Live while it records)
}

// ── formatting ─────────────────────────────────────────────────────────────────────────────
const clock = (s) => {
  s = Math.max(0, Math.round(s))
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), sec = s % 60
  const mm = String(m).padStart(h ? 2 : 1, '0'), ss = String(sec).padStart(2, '0')
  return h ? `${h}:${mm}:${ss}` : `${mm}:${ss}`
}
const hm = (d) => d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
const dayLabel = (d) => {
  const today = new Date(); today.setHours(0, 0, 0, 0)
  const that = new Date(d); that.setHours(0, 0, 0, 0)
  const diff = Math.round((today - that) / 86400000)
  if (diff === 0) return 'Today'
  if (diff === 1) return 'Yesterday'
  if (diff < 7) return that.toLocaleDateString([], { weekday: 'long' })
  return that.toLocaleDateString([], { day: 'numeric', month: 'long', year: today.getFullYear() === that.getFullYear() ? undefined : 'numeric' })
}
const STATUS = {
  recording: 'Recording', transcribing: 'Transcribing', notes: 'Notes', transcript: 'Transcript', failed: 'Failed', audio: 'Audio',
}
const speaker = (seg) => (seg.speaker === 'me' || seg.speaker === 'mic') ? 'Me' : (seg.name || 'Remote')

// ── safe Markdown: headings, paragraphs, nested + task lists, tables, quotes, rules, code
//    fences (```mermaid → a diagram), inline code / bold / italic / strike, https links, the
//    call's own frames as images, [mm:ss] anchors. Text is escaped before any markup is added.
function inline(text, ctx = {}) {
  // Code spans first: nothing inside them is markup.
  return String(text).split(/(`[^`]+`)/).map((part, i) => i % 2 ? `<code>${esc(part.slice(1, -1))}</code>` : rich(part, ctx)).join('')
}
function rich(text, ctx) {
  let s = esc(text)
  s = s.replace(/!\[([^\]]*)\]\((frames\/[\w.-]+\.(?:jpe?g|png))\)/g, (_, alt, path) => ctx.call
    ? `<button class="doc-img" data-frame="file/${encodeURIComponent(ctx.call)}/${path}"><img src="file/${encodeURIComponent(ctx.call)}/${path}" alt="${alt}" loading="lazy"></button>`
    : alt)
  s = s.replace(/\[([^\]]+)\]\((https:\/\/[^)\s]+)\)/g, (_, label, url) => `<a href="${url}" data-ext="1">${label}</a>`)
  s = s.replace(/\*\*([^*]+)\*\*|__([^_]+)__/g, (_, a, b) => `<strong>${a || b}</strong>`)
  s = s.replace(/(^|[^*\w])\*([^*\s][^*]*)\*/g, '$1<em>$2</em>')
  s = s.replace(/(^|[^\w])_([^_\s][^_]*)_(?!\w)/g, '$1<em>$2</em>')
  s = s.replace(/~~([^~]+)~~/g, '<del>$1</del>')
  s = s.replace(/\[((?:\d{1,2}:)?\d{1,2}:\d{2})\]/g, (_, t) => {
    const p = t.split(':').map(Number)
    const sec = p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1]
    return `<a class="at" data-at="${sec}" title="Jump to ${t} in the transcript">${t}</a>`
  })
  return s
}

const ITEM = /^(\s*)([-*+]|\d{1,9}[.)])\s+(.*)$/
const FENCE = /^\s*(`{3,}|~{3,})\s*([\w+-]*)/
const RULE = /^\s{0,3}([-*_])(\s*\1){2,}\s*$/
const TABLE_SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/
const cells = (line) => {
  let s = line.trim()
  if (s.startsWith('|')) s = s.slice(1)
  if (s.endsWith('|')) s = s.slice(0, -1)
  return s.split(/(?<!\\)\|/).map((c) => c.trim().replace(/\\\|/g, '|'))
}

// YAML frontmatter (the minutes format): `key: value` and `key:` + `  - item` lines, as a small
// two-column table. Values are shown as written, without their quotes.
const MD_KEY = /^([\w-]+):(?:\s+(.*))?$/
function mdFrontmatter(lines) {
  const rows = []
  for (const l of lines) {
    let m
    if ((m = l.match(MD_KEY))) rows.push([m[1], m[2] ? [m[2]] : []])
    else if ((m = l.match(/^\s+-\s+(.*)$/)) && rows.length) rows[rows.length - 1][1].push(m[1])
  }
  if (!rows.length) return ''
  const unquote = (v) => v.trim().replace(/^(["'])(.*)\1$/, '$2')
  return `<div class="table"><table class="frontmatter"><tbody>${
    rows.map(([k, v]) => `<tr><th>${esc(k)}</th><td>${esc(v.map(unquote).join(', '))}</td></tr>`).join('')}</tbody></table></div>`
}

function markdown(md, ctx = {}) {
  const lines = String(md).replace(/\r\n?/g, '\n').split('\n')
  const out = []
  let para = []
  const flush = () => { if (para.length) { out.push(`<p>${inline(para.join(' '), ctx)}</p>`); para = [] } }
  let i = 0
  if (lines[0].trim() === '---') {
    // Only a block of `key: value` lines (and, after a key, `  - item` lines) is frontmatter; else
    // the `---` is a rule and the block is content (as in calls::split_frontmatter).
    const end = lines.findIndex((l, k) => k > 0 && l.trim() === '---')
    const body = lines.slice(1, end)
    let key = false
    const yaml = body.every((l) => !l.trim() || (MD_KEY.test(l) ? (key = true) : key && /^\s+-\s/.test(l)))
    if (end > 1 && key && yaml) {
      out.push(mdFrontmatter(body))
      i = end + 1
    }
  }
  while (i < lines.length) {
    const line = lines[i]
    let m
    if ((m = line.match(FENCE))) {
      flush()
      const close = new RegExp(`^\\s*${m[1][0] === '`' ? '`' : '~'}{${m[1].length},}\\s*$`)
      const body = []
      for (i++; i < lines.length && !close.test(lines[i]); i++) body.push(lines[i])
      i++
      out.push(codeBlock(body.join('\n'), m[2].toLowerCase()))
      continue
    }
    if (!line.trim()) { flush(); i++; continue }
    if ((m = line.match(/^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/))) {
      flush()
      const n = m[1].length
      out.push(`<h${n}>${inline(m[2], ctx)}</h${n}>`)
      i++
      continue
    }
    if (RULE.test(line)) { flush(); out.push('<hr>'); i++; continue }
    if (/^\s{0,3}>/.test(line)) {
      flush()
      const inner = []
      for (; i < lines.length && /^\s{0,3}>/.test(lines[i]); i++) inner.push(lines[i].replace(/^\s{0,3}>\s?/, ''))
      out.push(`<blockquote>${markdown(inner.join('\n'), ctx)}</blockquote>`)
      continue
    }
    if (line.includes('|') && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1])) {
      flush()
      const head = cells(line)
      const align = cells(lines[i + 1]).map((c) => c.endsWith(':') ? (c.startsWith(':') ? 'center' : 'right') : '')
      const td = (tag, c, k) => `<${tag}${align[k] ? ` style="text-align:${align[k]}"` : ''}>${inline(c, ctx)}</${tag}>`
      const rows = []
      for (i += 2; i < lines.length && lines[i].includes('|') && lines[i].trim(); i++) rows.push(cells(lines[i]))
      out.push(`<div class="table"><table><thead><tr>${head.map((c, k) => td('th', c, k)).join('')}</tr></thead><tbody>${
        rows.map((r) => `<tr>${head.map((_, k) => td('td', r[k] || '', k)).join('')}</tr>`).join('')}</tbody></table></div>`)
      continue
    }
    if (ITEM.test(line)) { flush(); const [html, next] = mdList(lines, i, ctx); out.push(html); i = next; continue }
    para.push(line.trim())
    i++
  }
  flush()
  return out.join('')
}

/** A list and everything nested in it (by indentation), up to the first line that isn't part
 *  of it. Returns the HTML and the next line's index. */
function mdList(lines, i, ctx) {
  const root = { indent: -1, children: [] }
  const stack = [root]
  while (i < lines.length) {
    const line = lines[i]
    const m = line.match(ITEM)
    if (m && !RULE.test(line)) {
      const indent = m[1].replace(/\t/g, '    ').length
      while (stack.length > 1 && stack[stack.length - 1].indent >= indent) stack.pop()
      const node = { indent, ordered: /\d/.test(m[2]), start: parseInt(m[2], 10) || 1, text: [m[3]], children: [] }
      stack[stack.length - 1].children.push(node)
      stack.push(node)
      i++
      continue
    }
    if (!line.trim()) {
      // A blank line ends the list unless more of it follows.
      let j = i + 1
      while (j < lines.length && !lines[j].trim()) j++
      if (j < lines.length && (ITEM.test(lines[j]) || /^\s{2,}\S/.test(lines[j]))) { i = j; continue }
      break
    }
    if (/^\s+\S/.test(line) && stack.length > 1 && !FENCE.test(line)) { stack[stack.length - 1].text.push(line.trim()); i++; continue }
    break
  }
  return [mdItems(root.children, ctx), i]
}
function mdItems(nodes, ctx) {
  let html = ''
  for (let k = 0; k < nodes.length;) {
    const ordered = nodes[k].ordered
    const group = []
    while (k < nodes.length && nodes[k].ordered === ordered) group.push(nodes[k++])
    const tag = ordered ? 'ol' : 'ul'
    const start = ordered && group[0].start !== 1 ? ` start="${group[0].start}"` : ''
    html += `<${tag}${start}>${group.map((n) => {
      let text = n.text.join(' ')
      const task = text.match(/^\[([ xX])\]\s+(.*)$/)
      if (task) text = task[2]
      const box = task ? `<span class="check${task[1] !== ' ' ? ' done' : ''}" aria-hidden="true"></span>` : ''
      return `<li${task ? ' class="task-item"' : ''}>${box}${inline(text, ctx)}${n.children.length ? mdItems(n.children, ctx) : ''}</li>`
    }).join('')}</${tag}>`
  }
  return html
}

// Mermaid diagrams: drawn after the page is rendered (the library loads on first use), kept by
// source + theme so a background refresh shows the same SVG at once.
const diagrams = new Map()
// The system switching light/dark redraws them in the new colours.
matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => render())
const darkMode = () => {
  const t = document.documentElement.dataset.theme
  return t ? t === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches
}
function codeBlock(code, lang) {
  if (lang === 'mermaid') {
    const svg = diagrams.get(`${darkMode()}|${code}`)
    return svg
      ? `<figure class="diagram">${svg}</figure>`
      : `<figure class="diagram pending" data-src="${esc(code)}"><pre class="code"><code>${esc(code)}</code></pre></figure>`
  }
  return `<pre class="code"${lang ? ` data-lang="${esc(lang)}"` : ''}><code>${esc(code)}</code></pre>`
}
let mermaidReady = null
function loadMermaid() {
  if (!mermaidReady) {
    mermaidReady = new Promise((resolve, reject) => {
      const s = document.createElement('script')
      s.src = 'assets/mermaid.js'
      s.onload = () => resolve(window.mermaid)
      s.onerror = () => reject(new Error('could not load the diagram renderer'))
      document.head.appendChild(s)
    })
  }
  return mermaidReady
}
let diagramSeq = 0
async function drawDiagrams() {
  const pending = [...document.querySelectorAll('figure.diagram.pending:not([data-busy])')]
  if (!pending.length) return
  let mermaid
  try { mermaid = await loadMermaid() } catch { return }
  const dark = darkMode()
  const css = getComputedStyle(document.documentElement)
  const v = (name) => css.getPropertyValue(name).trim()
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict', // labels are text, no links or scripts in diagrams
    theme: 'base',
    fontFamily: getComputedStyle(document.body).fontFamily,
    themeVariables: {
      darkMode: dark,
      background: v('--surface') || (dark ? '#1b1b21' : '#ffffff'),
      primaryColor: dark ? '#2a2540' : '#efeaff',
      primaryBorderColor: v('--accent') || '#8b62ff',
      primaryTextColor: v('--text') || (dark ? '#ececf1' : '#18181d'),
      lineColor: v('--text-3') || '#74747f',
      secondaryColor: dark ? '#3a2f1a' : '#fff4e0',
      tertiaryColor: dark ? '#22222a' : '#f6f6f8',
      fontSize: '13px',
    },
    flowchart: { htmlLabels: false, curve: 'basis' },
  })
  for (const fig of pending) {
    fig.dataset.busy = '1'
    const src = fig.dataset.src
    try {
      const { svg } = await mermaid.render(`diagram-${++diagramSeq}`, src)
      diagrams.set(`${dark}|${src}`, svg)
      if (fig.isConnected) { fig.innerHTML = svg; fig.classList.remove('pending') }
    } catch (e) {
      fig.classList.remove('pending')
      fig.classList.add('failed')
      fig.insertAdjacentHTML('beforeend', `<figcaption>This diagram couldn't be drawn: ${esc(String((e && e.message) || e).split('\n')[0])}</figcaption>`)
    }
  }
}

// ── brand: the onetest logo (the full octopus character on its violet disc) ────────────────
const mark = (cls = '') => `<img class="logo ${cls}" src="assets/logo.png" alt="" aria-hidden="true">`

// ── data ───────────────────────────────────────────────────────────────────────────────────
async function loadState() {
  try { S.state = await get('api/state') } catch { /* app restarting */ }
}
async function loadCalls() {
  try { S.calls = await get('api/calls'); S.error = null } catch (e) { S.error = String(e) }
}
async function loadAgenda() {
  try { S.agenda = await get('api/agenda') } catch { S.agenda = [] }
}
async function loadCall(name) {
  try { S.call = await get('api/call/' + encodeURIComponent(name)) } catch (e) {
    // Retitled meanwhile (the notes run renames the folder): the same call keeps its stamp.
    const stamp = name.slice(0, 17)
    const moved = (S.calls || []).find((c) => c.name !== name && c.name.slice(0, 17) === stamp)
    if (moved && S.selected === name) { S.selected = moved.name; history.replaceState(null, '', `#call/${moved.name}/${S.tab}`); return loadCall(moved.name) }
    S.call = { error: String(e) }
  }
}

// ── routing: #calls, #call/<name>/<tab>, #today ────────────────────────────────────────────
function route() {
  const h = decodeURIComponent(location.hash.slice(1))
  const cm = h.match(/^calendar(?:\/(day|week))?$/)
  if (cm) { S.view = 'calendar'; if (cm[1]) S.cal.mode = cm[1]; return }
  S.view = 'calls'
  const m = h.match(/^call\/([^/]+)(?:\/(\w+))?$/)
  if (m) {
    if (S.selected !== m[1]) { S.selected = m[1]; S.call = null; S.confirmDelete = false; S.find = '' }
    if (m[2]) S.tab = m[2]
  }
}
const go = (hash) => { if (location.hash.slice(1) !== hash) location.hash = hash; else render() }

// ── language of the next recording (Record ▾) ──────────────────────────────────────────────
// Per viewer, remembered in this webview; English unless chosen otherwise. '' = auto: Echo
// detects among the configured languages (Settings → Languages).
const LANG_KEY = 'echo.recordLanguage'
function recordLang() {
  try { const v = localStorage.getItem(LANG_KEY); if (v !== null) return v } catch { /* storage off */ }
  return 'en'
}
function setRecordLang(v) { try { localStorage.setItem(LANG_KEY, v) } catch { /* storage off */ } }
const LANG_NAMES = (() => { try { return new Intl.DisplayNames(['en'], { type: 'language' }) } catch { return null } })()
const langName = (c) => { try { return (LANG_NAMES && LANG_NAMES.of(c)) || c.toUpperCase() } catch { return c.toUpperCase() } }
/** The languages on offer: English, then the configured ones; and the current choice. */
function langChoices() {
  const configured = (S.state && S.state.languages) || []
  const opts = [...new Set(['en', ...configured])]
  const cur = recordLang()
  return { configured, opts, cur: cur === '' || opts.includes(cur) ? cur : 'en' }
}
// ── AI profiles: which model (and account) writes a call's live notes and minutes ──────────
const ai = () => (S.state && S.state.ai && S.state.ai.profiles) ? S.state.ai : { profiles: [], default: null, live_notes: false, error: S.state && S.state.ai && S.state.ai.error }
const profileOf = (id) => ai().profiles.find((p) => p.id === id)
const profileName = (id) => { const p = profileOf(id); return p ? p.name : (id || '') }
/** Where a profile's requests go, in words. */
const profileWhere = (p) => !p ? '' : p.kind === 'local' ? (p.host ? `at ${p.host}` : 'on this computer') : p.kind === 'claude' ? `with your Claude account${p.account ? ` (${p.account})` : ''}` : 'as Claude Code is set up'
/** The profile shown for the next recording: the one picked here, else the default. */
function recordProfile() { const p = S.recordProfile; return p && profileOf(p) ? p : ai().default }
/** What a recording is sent with: only a profile picked here (else the app pins its default when
 *  the recording starts, and a broken AI setting never stops it). */
function pickedProfile() { const p = S.recordProfile; S.recordProfile = null; return p && profileOf(p) ? p : undefined }
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
const CARET = '<svg viewBox="0 0 10 6" width="8" height="5" aria-hidden="true"><path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.5"/></svg>'
/** A compact native select over a label (as the language picker): `data-<attr>`. */
function profilePick(attr, cur, cls = 'btn', title = 'Model for this call') {
  if (ai().profiles.length < 2) return ''
  const short = profileName(cur)
  return `<label class="${cls} split-lang pick" title="${esc(title)}: ${esc(profileLabel(profileOf(cur)) || short)}"><span>${esc(short.length > 14 ? short.slice(0, 13) + '…' : short)}</span>${CARET}<select ${attr} aria-label="${esc(title)}">${profileOptions(cur)}</select></label>`
}

/** A record button with the language picker on its right (a native select over the caret), and
 *  the AI profile picker when there is more than one. */
function recordButton(attrs, label, primary = true) {
  const { configured, opts, cur } = langChoices()
  const cls = primary ? 'btn primary' : 'btn'
  const auto = configured.length > 1 ? `Detect (${configured.map((c) => c.toUpperCase()).join(', ')})` : 'Detect the language'
  const options = opts.map((c) => `<option value="${esc(c)}"${c === cur ? ' selected' : ''}>${esc(langName(c))}</option>`).join('')
    + `<option value=""${cur === '' ? ' selected' : ''}>${esc(auto)}</option>`
  const title = cur ? `Transcribed as ${langName(cur)}` : 'Echo detects the language'
  return `<span class="split"><button class="${cls} split-main" ${attrs}>${esc(label)}</button><label class="${cls} split-lang" title="${esc(title)}"><span>${cur ? esc(cur.toUpperCase()) : 'Auto'}</span>${CARET}<select data-reclang aria-label="Language of the call">${options}</select></label>${profilePick('data-recprofile', recordProfile(), cls, 'Model for live notes and minutes')}</span>`
}

// ── views ──────────────────────────────────────────────────────────────────────────────────
function sidebar() {
  const st = S.state || {}
  const recording = !!st.recording, armed = !!st.armed
  const n = (S.calls || []).length
  const upcoming = (S.agenda || []).filter((m) => new Date(m.start) > new Date(Date.now() - 4 * 60000)).length
  const card = recording
    ? `<div class="status-card recording"><div class="status-line"><b>Recording</b><span>${esc(titleOf(st.recording))}</span></div><button class="btn" data-cmd="stop">Stop recording</button></div>`
    : armed
      ? `<div class="status-card armed"><div class="status-line"><b>Armed</b><span>Recording starts with the next Teams call</span></div><button class="btn" data-cmd="stop">Cancel</button></div>`
      : `<div class="status-card"><div class="status-line"><b>${st.in_call ? 'Teams call in progress' : 'Not recording'}</b><span>${st.in_call ? 'It is not being recorded' : 'Echo records only the calls you choose'}</span></div>${recordButton('data-cmd="record"', st.in_call ? 'Record this call' : 'Record next call')}${signinNote(profileOf(recordProfile()))}</div>`
  return `<aside class="sidebar">
    <div class="brand">${mark()}<b>Echo</b></div>
    <nav class="nav">
      <button data-go="calls" ${S.view === 'calls' ? 'aria-current="page"' : ''}><span class="label">Calls</span><span class="count">${n || ''}</span></button>
      <button data-go="calendar" ${S.view === 'calendar' ? 'aria-current="page"' : ''}><span class="label">Calendar</span><span class="count">${upcoming || ''}</span></button>
    </nav>
    ${card}
  </aside>`
}

function titleOf(dir) {
  if (!dir) return ''
  const name = String(dir).split(/[\\/]/).pop()
  const c = (S.calls || []).find((x) => x.name === name)
  return c ? c.title : name
}

function list() {
  const q = S.query.trim().toLowerCase()
  const calls = (S.calls || []).filter((c) => !q || c.title.toLowerCase().includes(q) || c.people.some((p) => p.toLowerCase().includes(q)))
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
      const meta = [d ? hm(d) : '', c.minutes ? `${c.minutes} min` : '', c.people.length ? c.people.slice(0, 2).join(', ') + (c.people.length > 2 ? ` +${c.people.length - 2}` : '') : ''].filter(Boolean).map((x) => `<span>${esc(x)}</span>`).join('')
      return `${head}<button class="row" role="option" data-call="${esc(c.name)}" aria-selected="${c.name === S.selected}">
        <span class="t">${esc(c.title)}</span>
        <span class="s"><span class="badge ${esc(c.status)}">${STATUS[c.status] || esc(c.status)}</span></span>
        <span class="m">${meta}</span>
      </button>`
    }).join('')
  }
  return `<section class="list">
    <div class="list-head"><h1>Calls</h1><input class="search" id="q" placeholder="Search titles and people" value="${esc(S.query)}" aria-label="Search calls"></div>
    <div class="rows" role="listbox" aria-label="Calls">${body}</div>
  </section>`
}

function emptyMain() {
  if (S.calls && !S.calls.length) {
    return `<div class="empty"><div>${mark()}<h3>No calls yet</h3><p>Use Record in the sidebar or the menu-bar octopus. Echo transcribes each call on this computer when it ends.</p></div></div>`
  }
  return `<div class="empty"><div>${mark()}<h3>Pick a call</h3><p>Its notes, transcript, chat and shared screens open here.</p></div></div>`
}

function doc() {
  if (!S.selected) return emptyMain()
  const c = S.call
  if (!c) return `<div class="pane"><div class="skeleton">${'<i></i>'.repeat(10)}</div></div>`
  if (c.error) return `<div class="empty"><div><h3>This call can't be opened</h3><p class="error">${esc(c.error)}</p></div></div>`
  const d = c.started_at ? new Date(c.started_at) : null
  const segs = Array.isArray(c.segments) ? c.segments : []
  const chat = Array.isArray(c.chat) ? c.chat : []
  const frames = Array.isArray(c.frames) ? c.frames : []
  const actions = Array.isArray(c.actions) ? c.actions : null
  const live = hasLive(c)
  const tabs = [
    live ? ['live', 'Live', ''] : null,
    ['notes', 'Notes', ''],
    ['transcript', 'Transcript', ''],
    chat.length ? ['chat', 'Chat', chat.length] : null,
    frames.length ? ['screens', 'Screens', frames.length] : null,
    ['actions', 'Actions', actions ? actions.length : ''],
  ].filter(Boolean)
  // While a call records (or is transcribed), open it on Live once; once its minutes are written,
  // on Notes once. The user's own choice stays.
  if (live && S.autoTab !== c.name && (c.status === 'recording' || c.status === 'transcribing')) { S.autoTab = c.name; S.tab = 'live' }
  if (S.autoTab === c.name && S.tab === 'live' && c.notes && c.notes_run && c.notes_run.state === 'done') { S.autoTab = 'done:' + c.name; S.tab = 'notes' }
  if (!tabs.some((t) => t[0] === S.tab)) S.tab = 'notes'
  const people = c.people && c.people.length ? `<span class="people">${esc(c.people.slice(0, 6).join(', '))}${c.people.length > 6 ? ` and ${c.people.length - 6} more` : ''}</span>` : ''
  return `<header class="doc-head">
      <div class="head-row">
        <div style="min-width:0">
          <h2 class="selectable">${esc(c.title)}</h2>
          <div class="facts">${d ? `<span>${esc(d.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' }))}, ${esc(hm(d))}</span>` : ''}${c.minutes ? `<span>${c.minutes} min</span>` : ''}${people}</div>
        </div>
        <div class="doc-actions">
          <button class="btn ghost" data-cmd="reveal">Show folder</button>
          <button class="btn ghost danger" data-cmd="ask-delete">Delete</button>
        </div>
      </div>
      <div class="tabs" role="tablist">${tabs.map(([id, label, n]) => `<button class="tab" role="tab" data-tab="${id}" aria-selected="${S.tab === id}">${label}${n !== '' ? `<span class="n">${n}</span>` : ''}</button>`).join('')}</div>
    </header>
    <div class="pane" id="pane">${S.confirmDelete ? confirmBox(c) : ''}${tabBody(c, segs, chat, frames, actions)}</div>`
}

function confirmBox(c) {
  return `<div class="confirm"><span>Delete "${esc(c.title)}"? It goes to the trash in the data folder for 30 days.</span>
    <button class="btn" data-cmd="cancel-delete">Keep</button><button class="btn primary" style="background:var(--danger);border-color:var(--danger)" data-cmd="delete">Delete</button></div>`
}

/** Transcript turns: consecutive lines of one speaker joined; `q` highlights, `jump` (seconds)
 *  marks the turn to scroll to. */
function turnsHtml(segs, q, jump) {
  const turns = []
  for (const s of segs) {
    const last = turns[turns.length - 1]
    if (last && last.who === speaker(s) && s.start - last.end < 8) { last.text += ' ' + s.text; last.end = s.end }
    else turns.push({ who: speaker(s), me: speaker(s) === 'Me', spk: s.speaker_id, start: s.start, end: s.end, lang: s.lang, text: s.text })
  }
  const jumpIdx = jump == null ? -1 : Math.max(0, turns.findIndex((t) => t.end >= jump - 2))
  return `<div class="transcript">${turns.map((t, i) => {
    const hit = (q && t.text.toLowerCase().includes(q)) || i === jumpIdx
    // Detected remote speakers each get a color of their own (speakers.rs numbers them).
    const spk = !t.me && t.spk ? ` spk${((t.spk - 1) % 6) + 1}` : ''
    return `<div class="turn ${t.me ? 'me' : 'remote'}${spk}${hit ? ' hit' : ''}" ${i === jumpIdx ? 'id="jump"' : ''}>
      <time>${clock(t.start)}</time>
      <div><div class="who">${esc(t.who)}${t.lang ? `<span class="lang">${esc(t.lang)}</span>` : ''}</div><p>${esc(t.text)}</p></div></div>`
  }).join('')}</div>`
}

function tabBody(c, segs, chat, frames, actions) {
  if (S.tab === 'transcript') {
    if (!segs.length) return c.status === 'recording'
      ? `<p style="color:var(--text-3)">Listening. The live transcript appears here as people speak.</p>`
      : `<p style="color:var(--text-3)">${c.status === 'transcribing' ? 'Transcribing on this computer. This takes about a quarter of the call length.' : 'No transcript for this call.'}</p>`
    return `<div class="from-bar"><input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript"></div>
      ${turnsHtml(segs, S.find.trim().toLowerCase(), S.jump)}`
  }
  if (S.tab === 'live') return liveTab(c, segs)
  if (S.tab === 'chat') {
    return `<div class="chat">${chat.map((m) => `<article class="msg"><header><b>${esc(m.from)}</b><time>${m.t == null ? '' : m.t < 0 ? 'before the call' : clock(m.t)}</time></header>
      <p>${esc(m.text)}</p>${m.files && m.files.length ? `<div class="files">${esc(m.files.join(', '))}</div>` : ''}</article>`).join('')}</div>`
  }
  if (S.tab === 'screens') {
    return `<div class="frames">${frames.map((f) => `<button class="frame" data-frame="${esc(f.url)}"><img src="${esc(f.url)}" alt="Shared screen at ${clock(f.t || 0)}" loading="lazy"><span>${clock(f.t || 0)}</span></button>`).join('')}</div>`
  }
  if (S.tab === 'actions') {
    if (!actions) return claudeHint(c, 'No action items yet', 'Action items are written together with the notes.')
    if (!actions.length) return `<p style="color:var(--text-3)">No action items in this call.</p>`
    return `<div class="todo">${actions.map((a, i) => {
      const done = a.status === 'done'
      return `<div class="item${done ? ' done' : ''}">
        <button class="check" role="checkbox" aria-checked="${done}" data-toggle="${i}" aria-label="Mark done"></button>
        <div><div class="task">${esc(a.task)}</div><div class="meta">
          <span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner || 'Unassigned')}</span>
          ${a.due ? `<span class="due">Due ${esc(a.due)}</span>` : ''}
          ${a.confidence === 'low' ? '<span class="uncertain">Uncertain</span>' : ''}
        </div></div>
        ${typeof a.t === 'number' ? `<button class="btn ghost" data-at="${a.t}">${clock(a.t)}</button>` : '<span></span>'}
      </div>`
    }).join('')}</div>`
  }
  // notes
  if (c.notes) return `<article class="doc">${markdown(c.notes, { call: c.name })}</article>`
  if (c.status === 'recording') return `<p style="color:var(--text-3)">Notes are written after the call. Follow it live in the ${hasLive(c) ? 'Live' : 'Transcript'} tab.</p>`
  return claudeHint(c, 'No notes yet', c.status === 'transcribing' ? 'Transcribing now. Notes come after that.' : 'The transcript is ready.')
}

/** No notes (or no action items) yet: Write notes runs Claude Code from the app (the same
 *  process-call skill a session runs); the run's state and log come with the call. */
function claudeHint(c, heading, lead) {
  const n = (S.state && S.state.notes) || {}
  const run = c.notes_run
  const cmd = `/echo:process-call ${c.name}`
  const log = c.has_log ? '<button class="btn ghost" data-cmd="show-log">Show log</button>' : ''
  const box = (h, p, buttons, busy) => `<div class="empty" style="height:auto;padding:40px 0"><div>${mark(busy ? 'busy' : '')}<h3>${esc(h)}</h3>
    ${p}${buttons ? `<div class="notes-actions">${buttons}</div>` : ''}</div></div>`
  if (run && run.state === 'queued') {
    return box('Notes are waiting their turn', `<p>${esc(run.message || 'They start as soon as the model is free.')}</p><p class="alt">With <b>${esc(run.model || '')}</b>. You can close this window.</p>`, '', true)
  }
  if (run && run.state === 'running') {
    return box('Writing notes…', `<p>Claude Code is writing the notes and action items with <b>${esc(run.model || n.model || 'sonnet')}</b>. It takes a few minutes; you can close this window.</p>`, log, true)
  }
  if (c.status === 'recording' || c.status === 'transcribing') return box(heading, `<p>${esc(lead)}</p>`, '')
  // A run refused for a Claude profile's sign-in (the built-in profile signs in in Terminal), and
  // that profile's sign-in now: signed in since (or a model refused it although it looks signed in).
  const signinOf = run && run.state === 'needs_login' && [profileOf(run.profile)].find((x) => x && x.kind === 'claude')
  const now = signinOf && signinOf.signin
  const again = now && now.state === 'signed_in'
  const why = {
    needs_login: again ? ['Ready to try again', run.message ? `The last try said: ${run.message}` : '']
      : [now && now.state === 'other_account' ? 'These notes need another account' : 'Claude Code needs you to sign in', run && run.message],
    failed: ["Couldn't write notes", run && run.message],
    skipped: ['No notes for this call', run && run.message],
  }[run && run.state]
  if (why) [heading, lead] = [why[0], why[1] || lead]
  const session = `<p class="alt">Or in a Claude Code session: <code>${esc(cmd)}</code> <button class="btn ghost" data-copy="${esc(cmd)}">Copy</button></p>`
  if (!n.claude) {
    return box(heading, `<p>${esc(lead)}</p><p>Notes and action items are written by Claude Code. Install it from <a href="https://claude.com/claude-code" data-ext="1">claude.com/claude-code</a>, then come back here.</p>`, log)
  }
  if (!n.plugin) {
    return box(heading, `<p>${esc(lead)}</p><p>Connect Claude Code first: menu → Claude Code → Connect Claude Code.</p>`, log)
  }
  const label = why && run.state !== 'skipped' ? 'Try again' : 'Write notes'
  // The call's own profile; none when it can't be read (the run would fail: say why, not the default).
  const live = c.live || {}, picked = S.notesProfile && S.notesProfile.call === c.name
  const pid = picked ? S.notesProfile.id : (live.profile || (live.profile_error ? '' : ai().default))
  const p = profileOf(pid)
  const model = p && p.notes_model ? p.notes_model : (n.model || 'sonnet')
  // The profile these notes need signed in: the call's profile now (picked here or in the Live tab);
  // the one the last run was refused for only while it still is (or none can be read). Signed in
  // now: Try again (or sign in again, if a model keeps refusing it).
  const target = signinOf && (!p || p.id === signinOf.id) ? signinOf : p
  const sign = signinNote(target) || (again && target === signinOf
    ? `<div class="signin"><span class="signin-text">${esc(signinOf.name)} is signed in as ${esc(now.account)} now — click Try again.</span><span class="signin-acts"><button class="btn ghost" data-signin="${esc(signinOf.id)}">Sign in again…</button></span></div>` : '')
  const where = ai().error ? `The AI settings can't be read: ${esc(ai().error)}`
    : !p && !picked && live.profile_error ? `This call's model can't be read: ${esc(live.profile_error)}`
    : !p ? (pid === '!unreadable' ? 'No model could be chosen when this call started — pick one.' : `The model this call was set to (${esc(pid || '?')}) no longer exists — pick another.`)
    : p.kind !== 'system'
      ? `Claude Code writes them with <b>${esc(p.name)}</b> (${esc(model)}), ${esc(profileWhere(p))}.`
      : `Claude Code writes them with <b>${esc(n.model || 'sonnet')}</b>, as it is set up (menu → Claude Code → Notes model).`
  const pick = ai().profiles.length > 1 || (ai().profiles.length && !p) ? `<label class="notes-pick">Model <select data-notesprofile aria-label="Model for these notes">${p ? '' : '<option value="" selected>Choose…</option>'}${profileOptions(pid)}</select></label>` : ''
  return box(heading, `<p>${esc(lead)}</p><p class="alt">${where}</p>${sign}`,
    `${pick}<button class="btn primary" data-cmd="write-notes">${label}</button>${log}`) + session
}

// ── Live tab: live notes, questions, last time (written by the app, `livenotes.rs`) ────────
/** A call has a Live tab when it has live notes or answers, or while it records or is transcribed with live notes on
 * (or the AI settings unreadable). */
function hasLive(c) {
  const l = c.live || {}, st = l.status || {}
  if (l.notes || (l.answers && l.answers.length)) return true // what was written stays readable
  return (!!st.enabled || !!st.settings_error) && (c.status === 'recording' || c.status === 'transcribing')
}

const LIVE_STATE = {
  waiting: 'Waiting for enough to summarise',
  updating: 'Updating…',
  updated: 'Updated',
  failed: 'The last update failed',
  busy: 'Waiting for the model to finish another job',
  signing_in: 'Waiting for the sign-in',
  paused: 'Paused',
  ended: 'The call ended',
}

function ago(iso) {
  if (!iso) return ''
  const s = Math.max(0, Math.round((Date.now() - new Date(iso)) / 1000))
  return s < 60 ? `${s} s ago` : `${Math.round(s / 60)} min ago`
}

const atLink = (t) => typeof t === 'number' ? ` <a class="at" data-at="${t}" title="Jump to ${clock(t)} in the transcript">${clock(t)}</a>` : ''

function liveHead(c) {
  const l = c.live || {}, st = l.status || {}, n = l.notes
  const state = st.paused ? 'paused' : st.state || (c.status === 'recording' ? 'waiting' : 'ended')
  let line = LIVE_STATE[state] || ''
  if (state === 'updated' && n && n.updated_at) line = `Updated ${ago(n.updated_at)}`
  const p = profileOf(l.profile)
  const model = ai().profiles.length > 1
    ? `<label class="btn split-lang pick live-model" title="Model for this call's live notes and minutes"><span>${esc(profileName(l.profile))}</span>${CARET}<select data-callprofile aria-label="Model for this call">${profileOptions(l.profile)}</select></label>`
    : `<span class="live-model">${esc(profileName(l.profile))}</span>`
  const where = p ? `<span class="hint">${esc(profileWhere(p))}</span>` : ''
  const draftBy = n && n.profile && l.profile && n.profile !== l.profile ? `<span class="hint">Draft by ${esc(profileName(n.profile))}; the next update by ${esc(profileName(l.profile))}</span>` : ''
  const pause = c.status === 'recording'
    ? `<button class="btn ghost" data-cmd="${st.paused ? 'live-resume' : 'live-pause'}">${st.paused ? 'Resume' : 'Pause'}</button>` : ''
  // A sign-in problem (or one going on) for this call's profile: said with what to do about it.
  const failedFor = st.needs_sign_in && profileOf(st.needs_sign_in)
  const sign = failedFor ? (signinNote(failedFor) || `<div class="signin error"><span class="signin-text">${esc(st.error || '')}</span><span class="signin-acts"><button class="btn primary" data-signin="${esc(failedFor.id)}">Sign in…</button></span></div>`)
    : signinNote(p)
  const problem = st.settings_error ? `The AI settings can't be read: ${st.settings_error}` : !st.enabled ? 'Live notes are off (menu → AI for calls → Live notes during calls).' : (!failedFor && st.state === 'failed' && st.error) || ''
  return `<div class="live-head">${model}${where}<span class="live-state ${esc(state)}">${esc(line)}</span>${draftBy}<span class="grow"></span>${pause}</div>
    ${sign}${problem ? `<p class="live-error">${esc(problem)}</p>` : ''}`
}

function liveNotes(c) {
  const l = c.live || {}, n = l.notes, st = l.status || {}
  if (!n) {
    if (c.status !== 'recording') return '<p class="muted">No live notes for this call.</p>'
    // Why there are none yet (the reason itself is in the header above).
    const why = st.paused ? 'Paused — no notes until you resume.'
      : st.needs_sign_in ? `No notes until ${profileName(st.needs_sign_in)} can be used (see above).`
      : st.state === 'signing_in' ? 'No notes until the sign-in is done.'
      : st.state === 'failed' ? 'No notes yet: the last update failed (see above).'
      : st.state === 'busy' ? 'No notes yet: the model is finishing another job.'
      : 'The first notes appear after about 90 seconds of conversation.'
    return `<p class="muted">${esc(why)}</p>`
  }
  const sec = (title, items, f) => items && items.length ? `<h3>${title}</h3><ul>${items.map(f).join('')}</ul>` : ''
  const gaps = (n.gaps || []).map((g) => `${clock(g[0])}–${clock(g[1])}`).join(', ')
  const empty = !n.now && !(n.decisions || []).length && !(n.actions || []).length && !(n.questions || []).length
  return `${n.now ? `<p class="live-now"><b>Now</b> ${esc(n.now)}</p>` : ''}
    ${sec('Decisions', n.decisions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${sec('Action items', n.actions, (a) => `<li><span class="owner${a.owner === 'Me' ? ' me' : ''}">${esc(a.owner)}</span> ${esc(a.task)}${atLink(a.t)}</li>`)}
    ${sec('Open questions', n.questions, (x) => `<li>${esc(x.text)}${atLink(x.t)}</li>`)}
    ${empty ? '<p class="muted">Nothing decided, owed or open yet.</p>' : ''}
    ${gaps ? `<p class="muted">Not covered: ${esc(gaps)}</p>` : ''}`
}

function prepCard(c) {
  const p = (c.live || {}).prep
  if (!p || (!(p.open_actions || []).length && !(p.highlights || []).length)) return ''
  const a = p.open_actions || [], h = p.highlights || []
  const mine = S.prepOpen[c.name]
  const open = (mine !== undefined ? mine : c.status === 'recording' && !(c.live.notes && c.live.notes.updated_at)) ? ' open' : ''
  return `<details class="prep" data-prep="${esc(c.name)}"${open}><summary>Last time${p.date ? ` (${esc(p.date)})` : ''}: ${a.length} open action${a.length === 1 ? '' : 's'}, ${h.length} highlight${h.length === 1 ? '' : 's'}</summary>
    ${a.length ? `<h3>Open actions</h3><ul>${a.map((x) => `<li>${x.owner ? `<span class="owner">${esc(x.owner)}</span> ` : ''}${esc(x.task)}${x.due ? ` <span class="muted">due ${esc(x.due)}</span>` : ''}</li>`).join('')}</ul>` : ''}
    ${h.length ? `<h3>Highlights</h3><ul>${h.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : ''}
  </details>`
}

/** An answer: escaped, line breaks kept, "* " / "- " bullets as •, [mm:ss] as jumps. */
function answerText(t) {
  return esc(String(t).replace(/^\s*[*-]\s+/gm, '• ')).replace(/\[((?:\d{1,2}:)?\d{1,2}:\d{2})\]/g, (_, x) => {
    const p = x.split(':').map(Number)
    const sec = p.length === 3 ? p[0] * 3600 + p[1] * 60 + p[2] : p[0] * 60 + p[1]
    return `<a class="at" data-at="${sec}" title="Jump to ${x} in the transcript">${x}</a>`
  }).replace(/\n/g, '<br>')
}

const ASK_TITLE = { missed: 'What did I miss?', question: 'Questions you could ask', ask: 'You asked' }
/** What a question put back waits for (`asking.waiting`). */
const ASK_WAIT = { model: 'Waiting for the model to finish another job…', sign_in: 'Waiting for the sign-in to finish…' }

function liveAnswers(c) {
  const l = c.live || {}, st = l.status || {}
  const pending = st.asking
    ? `<div class="answer pending"><b>${esc(ASK_TITLE[st.asking.kind] || '')}</b>${st.asking.text ? ` ${esc(st.asking.text)}` : ''}<p class="muted">${ASK_WAIT[st.asking.waiting] || 'Thinking…'}</p></div>`
    : st.queued > 0 ? `<div class="answer pending"><p class="muted">Your question is next.</p></div>` : ''
  const list = (l.answers || []).slice().reverse().slice(0, 30).map((a) => `<div class="answer">
      <div class="answer-head"><b>${esc(ASK_TITLE[a.kind] || '')}</b>${a.question ? ` <span>${esc(a.question)}</span>` : ''}<span class="muted">${esc(hm(new Date(a.asked_at)))}</span></div>
      ${a.error ? `<p class="error">${esc(a.error)}</p>` : `<p>${answerText(a.answer)}</p>`}</div>`).join('')
  return pending + list
}

function liveTab(c, segs) {
  const st = (c.live || {}).status || {}
  const busy = !!st.asking || st.queued > 0
  const dis = busy ? ' disabled' : ''
  return `<div class="live">
    ${liveHead(c)}
    <div class="live-grid">
      <section class="live-transcript" id="live-transcript" aria-label="Live transcript">${segs.length ? turnsHtml(segs, '', null) : `<p class="muted">Listening. The transcript appears here as people speak.</p>`}</section>
      <section class="live-side" aria-label="Live notes">
        ${prepCard(c)}
        <div class="live-notes doc">${liveNotes(c)}</div>
        <div class="live-ask">
          <div class="live-buttons"><button class="btn" data-ask="missed"${dis}>What did I miss?</button><button class="btn" data-ask="question"${dis}>Suggest a question</button></div>
          <form class="ask-row" data-askform="1"><input id="ask" class="search" placeholder="Ask about this call…" value="${esc(S.liveDraft)}" autocomplete="off" maxlength="500" aria-label="Ask about this call"><button class="btn primary" type="submit"${dis}>Ask</button></form>
        </div>
        <div class="live-answers">${liveAnswers(c)}</div>
      </section>
    </div>
  </div>`
}

// ── calendar: today, or a rolling week (yesterday, today, 5 days ahead) ───────────────────
// Days and hours by the local calendar and clock, never by adding 24 h: a day the clocks change
// isn't 24 hours long (a week across one showed a day twice and drew the later days' events at
// midnight).
const ymd = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const midnight = (d) => { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
const addDays = (d, n) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n)
const sameDay = (a, b) => midnight(a).getTime() === midnight(b).getTime()
/** Where time `t` (ms) is on `day`'s clock, in hours: 0 before the day, 24 after it. */
const clockHours = (t, day) => {
  const d = new Date(t)
  return sameDay(d, day) ? d.getHours() + d.getMinutes() / 60 + d.getSeconds() / 3600 : t < day.getTime() ? 0 : 24
}

/** Side-by-side lanes for overlapping blocks in one day column, by their drawn hours `y0`..`y1`. */
function lanes(items) {
  items.sort((a, b) => a.y0 - b.y0 || b.y1 - a.y1)
  const cols = []
  let group = [], groupEnd = 0
  const flush = () => { const n = Math.max(1, ...group.map((x) => x.lane + 1)); group.forEach((x) => (x.lanes = n)); group = [] }
  for (const it of items) {
    if (group.length && it.y0 >= groupEnd) { flush(); cols.length = 0 }
    let lane = cols.findIndex((end) => end <= it.y0)
    if (lane < 0) { lane = cols.length; cols.push(0) }
    cols[lane] = it.y1
    it.lane = lane
    group.push(it)
    groupEnd = Math.max(groupEnd, it.y1)
  }
  if (group.length) flush()
  return items
}
/** One day column's blocks (`s`, `real`: start and end, ms) where they are drawn: `y0`..`y1` in
 *  clock hours on `day`, at least 20 min so a short meeting is still a readable block, in lanes by
 *  those hours (the hour a day the clocks go back repeats would put two meetings an hour apart in
 *  one place). Counted in whole seconds, so a block ending when the next starts never overlaps it. */
const layoutDay = (items, day) => lanes(items.map((x) => {
  const sec = (t) => Math.round(clockHours(t, day) * 3600)
  const s0 = sec(x.s)
  return Object.assign(x, { y0: s0 / 3600, y1: Math.max(sec(x.real), s0 + 1200) / 3600 })
}))
// end of the calendar's helpers (calendar-dates.test.mjs runs app.js from `const ymd` to here)
function calRange() {
  const today = midnight(new Date())
  if (S.cal.mode === 'day') return { start: addDays(today, S.cal.offset), days: 1 }
  return { start: addDays(today, S.cal.offset * 7 - 1), days: 7 }
}
async function loadCalendar(fresh) {
  const { start, days } = calRange()
  S.cal.loading = true
  try {
    S.cal.data = await get(`api/calendar?start=${ymd(start)}&days=${days}${fresh ? '&fresh=1' : ''}`)
  } catch (e) { S.cal.data = { error: String(e), events: [], calls: [] } }
  S.cal.loading = false
}

function eventState(e, now) {
  const s = new Date(e.start), en = new Date(e.end)
  const past = en < now, live = s <= now && en > now
  const recordable = e.online && !e.declined && !e.cancelled && !e.no_notetakers && !past
  return { s, en, past, live, recordable }
}

function calendarView() {
  const d = S.cal.data
  const { start, days } = calRange()
  const now = new Date()
  const dayList = Array.from({ length: days }, (_, i) => addDays(start, i))
  const title = days === 1
    ? start.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })
    : `${dayList[0].toLocaleDateString([], { day: 'numeric', month: 'short' })} to ${dayList[days - 1].toLocaleDateString([], { day: 'numeric', month: 'short', year: 'numeric' })}`
  const head = `<div class="cal-head"><h1>${esc(title)}</h1>
    <div class="seg" role="group" aria-label="View"><button data-calmode="day" aria-pressed="${S.cal.mode === 'day'}">Day</button><button data-calmode="week" aria-pressed="${S.cal.mode === 'week'}">Week</button></div>
    <button class="btn ghost" data-calnav="-1" aria-label="Earlier">‹</button><button class="btn" data-calnav="0">Today</button><button class="btn ghost" data-calnav="1" aria-label="Later">›</button></div>`
  let note = ''
  if (d && d.error) note = `<div class="cal-note error">Can't read the calendar: ${esc(d.error)}</div>`
  else if (d && !d.signed_in) note = `<div class="cal-note">Showing today's meetings only. Connect Microsoft 365 from the menu-bar octopus (Calendar) to see your week.</div>`
  if (!d) return `<div class="cal">${head}${note}<div style="padding:20px"><div class="skeleton">${'<i></i>'.repeat(8)}</div></div></div>`

  // Blocks per day.
  const blocks = dayList.map(() => [])
  const allday = dayList.map(() => [])
  for (const e of d.events || []) {
    const st = eventState(e, now)
    const di = dayList.findIndex((x) => sameDay(x, st.s))
    if (di < 0) continue
    if (e.all_day) { allday[di].push(e); continue }
    blocks[di].push({ kind: 'event', ev: e, st, s: st.s.getTime(), real: st.en.getTime() })
  }
  for (const c of d.calls || []) {
    const s = new Date(c.start), en = c.end ? new Date(c.end) : new Date(s.getTime() + 30 * 60000)
    const di = dayList.findIndex((x) => sameDay(x, s))
    if (di >= 0) blocks[di].push({ kind: 'call', c, s: s.getTime(), real: en.getTime() })
  }
  blocks.forEach((b, di) => layoutDay(b, dayList[di]))

  // The whole day, scrolling; an hour is sized so the working day (08:00–19:00) fills the
  // window (min 40 px).
  const h0 = 0, h1 = 24
  const reserved = 112 + (allday.some((a) => a.length) ? 30 : 0) + (note ? 34 : 0)
  const HOUR = Math.max(40, Math.floor((window.innerHeight - reserved) / 11))
  const cols = `grid-template-columns: repeat(${days}, minmax(0, 1fr))`
  const dayHead = `<div class="days" style="${cols}">${dayList.map((x) => `<div class="${sameDay(x, now) ? 'today' : ''}">${esc(x.toLocaleDateString([], { weekday: 'short' }))}<b>${x.getDate()}</b></div>`).join('')}</div>`
  const alldayRow = allday.some((a) => a.length) ? `<div class="allday" style="${cols}">${allday.map((a) => `<div>${a.map((e) => `<span title="${esc(e.subject)}">${esc(e.subject)}</span>`).join('')}</div>`).join('')}</div>` : ''
  const hourLabels = Array.from({ length: h1 - h0 }, (_, i) => `<span style="top:${i * HOUR}px">${i === 0 ? '' : String(h0 + i).padStart(2, '0') + ':00'}</span>`).join('')
  const top = (hours) => (hours - h0) * HOUR
  const colHtml = dayList.map((day, di) => {
    const lines = Array.from({ length: h1 - h0 }, (_, i) => `<div class="hourline" style="top:${i * HOUR}px"></div>`).join('')
    const nowLine = sameDay(day, now) ? `<div class="now" style="top:${top(clockHours(now.getTime(), day))}px"></div>` : ''
    const items = blocks[di].map((x) => {
      const y = top(x.y0), h = Math.max(22, top(x.y1) - y - 2)
      const w = 100 / x.lanes, left = x.lane * w
      const style = `top:${y}px;height:${h}px;left:calc(${left}% + 2px);width:calc(${w}% - 4px)`
      if (x.kind === 'call') {
        return `<button class="ev call" style="${style}" data-call="${esc(x.c.name)}"><b>${esc(x.c.title)}</b><small><span class="tag rec">Recorded</span> ${esc(hm(new Date(x.s)))}</small></button>`
      }
      const { ev: e, st } = x
      const cls = ['ev', e.online ? 'online' : '', e.recorded ? 'recorded' : '', e.scheduled ? 'scheduled' : '', st.past ? 'past' : '', e.declined ? 'declined' : '', e.cancelled ? 'cancelled' : ''].filter(Boolean).join(' ')
      const tag = e.recorded ? '<span class="tag rec">Recorded</span>' : e.scheduled ? '<span class="tag will">Will record</span>' : e.no_notetakers ? '<span class="tag no">No notetakers</span>' : ''
      return `<button class="${cls}" style="${style}" data-ev="${esc(e.id || '')}" aria-selected="${S.cal.selected === e.id}"><b>${esc(e.subject)}</b><small>${esc(hm(st.s))}${tag ? ' ' + tag : ''}</small></button>`
    }).join('')
    return `<div class="col${sameDay(day, now) ? ' today' : ''}" style="height:${(h1 - h0) * HOUR}px">${lines}${nowLine}${items}</div>`
  }).join('')
  const empty = !blocks.some((b) => b.length) && !allday.some((a) => a.length)
    ? `<p style="position:absolute;top:40%;left:0;right:0;text-align:center;color:var(--text-3);pointer-events:none">${days === 1 ? 'Nothing on the calendar this day.' : 'Nothing on the calendar these days.'}</p>` : ''
  const grid = `<div class="grid-wrap" id="calgrid" data-hour="${HOUR}"><div class="grid" style="grid-template-columns: 56px repeat(${days}, minmax(0, 1fr))"><div class="hours" style="height:${(h1 - h0) * HOUR}px">${hourLabels}</div>${colHtml}</div>${empty}</div>`
  const sel = S.cal.selected && (d.events || []).find((e) => e.id === S.cal.selected)
  return `<div class="cal">${head}${note}<div style="display:grid;grid-template-rows:auto auto 1fr;min-height:0">${dayHead}${alldayRow}${grid}</div>${sel ? eventDetail(sel, now) : ''}</div>`
}

function eventDetail(e, now) {
  const st = eventState(e, now)
  const when = `${st.s.toLocaleDateString([], { weekday: 'long', day: 'numeric', month: 'long' })}, ${hm(st.s)} to ${hm(st.en)}`
  let acts = ''
  if (e.recorded) acts = `<button class="btn primary" data-call="${esc(e.recorded.name)}">Open notes</button>`
  else if (st.recordable && e.scheduled) acts = `<button class="btn" data-unschedule="${esc(e.id)}">Don't record</button><p class="why">Echo arms itself a minute before it starts${e.scheduled_languages ? `, transcribed as ${esc(e.scheduled_languages.map(langName).join(', '))}` : ''}.</p>`
  else if (st.recordable && st.live && e.join_url) acts = `${recordButton(`data-join="${esc(e.id)}"`, 'Join and record')}<button class="btn" data-schedule="${esc(e.id)}">Record it</button>`
  else if (st.recordable) acts = recordButton(`data-schedule="${esc(e.id)}"`, st.live ? 'Record it' : 'Record when it starts')
  const why = e.recorded ? '' : e.no_notetakers ? 'The organizer asked for no notetakers.' : e.declined ? 'You declined this meeting.' : e.cancelled ? 'This meeting was cancelled.' : !e.online ? 'Not a Teams meeting.' : st.past ? 'This meeting is over and was not recorded.' : ''
  return `<aside class="detail">
    <button class="btn ghost close" data-calclose="1" aria-label="Close">Close</button>
    <h2 class="selectable">${esc(e.subject)}</h2><div class="when">${esc(when)}</div>
    <dl>${e.organizer ? `<dt>Organizer</dt><dd>${esc(e.organizer)}</dd>` : ''}${e.people ? `<dt>People</dt><dd>${e.people}</dd>` : ''}${e.location ? `<dt>Where</dt><dd>${esc(e.location)}</dd>` : ''}${e.recorded ? `<dt>Recording</dt><dd>${STATUS[e.recorded.status] || esc(e.recorded.status)}${e.recorded.minutes ? `, ${e.recorded.minutes} min` : ''}</dd>` : ''}</dl>
    ${acts ? `<div class="acts">${acts}</div>` : ''}${why ? `<p class="why">${esc(why)}</p>` : ''}
    ${e.recap && e.recap.length ? `<div><b style="font-size:12px">Last time</b><ul>${e.recap.map((r) => `<li>${esc(r)}</li>`).join('')}</ul></div>` : ''}
  </aside>`
}

// ── render + events ────────────────────────────────────────────────────────────────────────
let lastKey = ''
let lastPrint = ''
function render() {
  // Animate entering a view / call / tab, never a background refresh.
  const key = `${S.view}|${S.selected}|${S.tab}`
  const entering = key !== lastKey
  lastKey = key
  const focus = document.activeElement && document.activeElement.id
  const caret = focus && document.activeElement.selectionStart
  const scroll = $('#pane') ? $('#pane').scrollTop : 0
  // The live transcript follows the call while it is scrolled to the end.
  const lt = $('#live-transcript')
  const ltStick = !lt || lt.scrollTop + lt.clientHeight >= lt.scrollHeight - 40, ltTop = lt ? lt.scrollTop : 0
  const app = $('#app')
  if (S.view === 'calendar') {
    const grid = $('#calgrid'), keep = grid ? grid.scrollTop : null
    app.innerHTML = `<div class="shell no-list">${sidebar()}<main class="main">${calendarView()}</main></div>`
    const g = $('#calgrid')
    // First open: around now, or 08:00 on a week without today.
    if (g) g.scrollTop = keep != null ? keep : Math.max(0, ($('.now') ? $('.now').offsetTop - 120 : 8 * Number(g.dataset.hour)))
  } else {
    app.innerHTML = `<div class="shell">${sidebar()}${list()}<main class="main">${doc()}</main></div>`
  }
  if (focus && $('#' + focus)) { const el = $('#' + focus); el.focus(); try { el.setSelectionRange(caret, caret) } catch {} }
  const pane = $('#pane')
  if (pane) {
    const j = $('#jump')
    if (S.jump != null && j) { j.scrollIntoView({ block: 'center' }); S.jump = null }
    else pane.scrollTop = scroll
  }
  const lt2 = $('#live-transcript')
  if (lt2) lt2.scrollTop = ltStick ? lt2.scrollHeight : ltTop
  if (S.lightbox) app.insertAdjacentHTML('beforeend', `<div class="lightbox" data-close="1"><img src="${esc(S.lightbox)}" alt="Shared screen"></div>`)
  if (entering) { const p = $('.pane') || $('.grid-wrap'); if (p) p.classList.add('enter') }
  placeDetail()
  const g = $('#calgrid')
  if (g) g.addEventListener('scroll', placeDetail, { passive: true })
  lastPrint = fingerprint()
  drawDiagrams()
}

/** Put the event card next to the event it belongs to: right of it if there is room, else
 *  left; level with it, kept inside the window. */
function placeDetail() {
  const card = $('.detail'), ev = $('.ev[aria-selected="true"]')
  if (!card || !ev) return
  const r = ev.getBoundingClientRect(), w = card.offsetWidth, h = card.offsetHeight, gap = 8, pad = 12
  const right = r.right + gap + w <= window.innerWidth - pad
  const left = right ? r.right + gap : Math.max(pad, r.left - gap - w)
  const top = Math.min(Math.max(pad, r.top), Math.max(pad, window.innerHeight - h - pad))
  card.style.left = `${left}px`
  card.style.right = 'auto'
  card.style.top = `${top}px`
}

/** What the screen depends on: re-render only when it changes (keeps selection and scroll). */
const fingerprint = () => JSON.stringify([S.state, S.calls, S.call, S.agenda, S.cal.data, S.cal.selected])

document.addEventListener('click', (e) => {
  const t = e.target.closest('[data-go],[data-call],[data-tab],[data-cmd],[data-at],[data-toggle],[data-frame],[data-close],[data-copy],[data-arm],[data-join],[data-ev],[data-calmode],[data-calnav],[data-calclose],[data-schedule],[data-unschedule],[data-ask],[data-signin],[data-signin-cancel],[data-signin-accept],[data-signin-check],a[data-ext]')
  if (!t) return
  // Sign-ins (`signin.rs`): shown at once (starting / stopping / checking) until the next poll
  // brings the app's state.
  const busySignin = (id, state) => { const p = profileOf(id); if (p) { p.signin = { state }; render() } }
  if (t.dataset.signin) { send({ cmd: 'sign_in', profile: t.dataset.signin }); return busySignin(t.dataset.signin, 'starting') }
  if (t.dataset.signinCancel) { send({ cmd: 'sign_in_cancel', profile: t.dataset.signinCancel }); return busySignin(t.dataset.signinCancel, 'stopping') }
  if (t.dataset.signinAccept) { send({ cmd: 'accept_account', profile: t.dataset.signinAccept, account: t.dataset.account }); return busySignin(t.dataset.signinAccept, 'checking') }
  if (t.dataset.signinCheck) { send({ cmd: 'check_sign_in', profile: t.dataset.signinCheck }); return busySignin(t.dataset.signinCheck, 'checking') }
  if (t.dataset.ext) { e.preventDefault(); send({ cmd: 'open_url', url: t.getAttribute('href') }); return }
  if (t.dataset.go) return go(t.dataset.go === 'calls' ? (S.selected ? `call/${S.selected}/${S.tab}` : 'calls') : `calendar/${S.cal.mode}`)
  if (t.dataset.ev !== undefined) { S.cal.selected = S.cal.selected === t.dataset.ev ? null : t.dataset.ev; return render() }
  if (t.dataset.calclose) { S.cal.selected = null; return render() }
  if (t.dataset.calmode) { S.cal.mode = t.dataset.calmode; S.cal.offset = 0; S.cal.data = null; go(`calendar/${S.cal.mode}`); return loadCalendar().then(render) }
  if (t.dataset.calnav !== undefined) { const n = Number(t.dataset.calnav); S.cal.offset = n === 0 ? 0 : S.cal.offset + n; S.cal.data = null; render(); return loadCalendar().then(render) }
  if (t.dataset.schedule) { send({ cmd: 'schedule', id: t.dataset.schedule, languages: recordLang(), profile: pickedProfile() }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.schedule); if (e) e.scheduled = true; return render() }
  if (t.dataset.unschedule) { send({ cmd: 'unschedule', id: t.dataset.unschedule }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.unschedule); if (e) e.scheduled = false; return render() }
  if (t.dataset.call) return go(`call/${t.dataset.call}/${S.tab}`)
  if (t.dataset.tab) return go(`call/${S.selected}/${t.dataset.tab}`)
  if (t.dataset.at) { S.jump = Number(t.dataset.at); S.find = ''; return go(`call/${S.selected}/transcript`) }
  if (t.dataset.frame) { S.lightbox = t.dataset.frame; return render() }
  if (t.dataset.close) { S.lightbox = null; return render() }
  if (t.dataset.copy) { send({ cmd: 'copy', text: t.dataset.copy }); t.textContent = 'Copied'; return }
  if (t.dataset.toggle) {
    const a = S.call && S.call.actions && S.call.actions[Number(t.dataset.toggle)]
    if (a) { a.status = a.status === 'done' ? 'todo' : 'done'; render() } // optimistic; the file follows
    return send({ cmd: 'toggle_action', call: S.selected, index: Number(t.dataset.toggle) })
  }
  if (t.dataset.arm !== undefined) return send({ cmd: 'arm_meeting', id: t.dataset.arm, languages: recordLang(), profile: pickedProfile() })
  if (t.dataset.join !== undefined) return send({ cmd: 'join_meeting', id: t.dataset.join, languages: recordLang(), profile: pickedProfile() })
  if (t.dataset.ask) { send({ cmd: 'live_ask', call: S.selected, kind: t.dataset.ask }); return liveBusy() }
  switch (t.dataset.cmd) {
    case 'record': return send({ cmd: 'record', languages: recordLang(), profile: pickedProfile() })
    case 'stop': return send({ cmd: 'stop' })
    case 'reveal': return send({ cmd: 'reveal', call: S.selected })
    case 'show-log': return send({ cmd: 'show_log', call: S.selected })
    case 'write-notes': {
      const pid = S.notesProfile && S.notesProfile.call === S.selected && S.notesProfile.id ? S.notesProfile.id : null
      send({ cmd: 'write_notes', call: S.selected, profile: pid || undefined })
      if (S.call) { S.call.notes_run = { state: 'queued', model: profileName(pid || (S.call.live && S.call.live.profile)), message: 'Starting…' }; render() }
      return
    }
    case 'live-pause': return send({ cmd: 'live_pause', call: S.selected })
    case 'live-resume': return send({ cmd: 'live_resume', call: S.selected })
    case 'ask-delete': S.confirmDelete = true; return render()
    case 'cancel-delete': S.confirmDelete = false; return render()
    case 'delete': {
      const name = S.selected
      S.confirmDelete = false; S.selected = null; S.call = null
      S.calls = (S.calls || []).filter((c) => c.name !== name)
      send({ cmd: 'delete', call: name })
      return go('calls')
    }
  }
})

document.addEventListener('input', (e) => {
  if (e.target.id === 'q') { S.query = e.target.value; render() }
  if (e.target.id === 'find') { S.find = e.target.value; render() }
  if (e.target.id === 'ask') S.liveDraft = e.target.value // no render: typing must not be disturbed
})

document.addEventListener('change', (e) => {
  if (e.target.matches('[data-reclang]')) { setRecordLang(e.target.value); render() }
  // A profile chosen for the next recording or for these notes: a sign-in starts if it is plainly
  // signed out (any choice, of any kind, supersedes an earlier one's check).
  if (e.target.matches('[data-recprofile]')) { S.recordProfile = e.target.value; send({ cmd: 'sign_in_if_needed', profile: e.target.value, purpose: 'record' }); render() }
  if (e.target.matches('[data-notesprofile]')) { S.notesProfile = { call: S.selected, id: e.target.value }; send({ cmd: 'sign_in_if_needed', profile: e.target.value, purpose: `notes:${S.selected}` }); render() }
  if (e.target.matches('[data-callprofile]')) {
    send({ cmd: 'set_profile', call: S.selected, profile: e.target.value })
    if (S.call && S.call.live) { S.call.live.profile = e.target.value; render() }
  }
})

document.addEventListener('toggle', (e) => {
  if (e.target.matches && e.target.matches('details[data-prep]')) S.prepOpen[e.target.dataset.prep] = e.target.open
}, true)

document.addEventListener('submit', (e) => {
  if (!e.target.matches('[data-askform]')) return
  e.preventDefault()
  const text = S.liveDraft.trim()
  if (!text) return
  send({ cmd: 'live_ask', call: S.selected, kind: 'ask', text })
  S.liveDraft = ''
  liveBusy()
})

/** A question was sent: show it as pending until the next poll brings the app's state. */
function liveBusy() {
  const st = S.call && S.call.live && S.call.live.status
  if (st) st.queued = (st.queued || 0) + 1
  render()
}

document.addEventListener('keydown', (e) => {
  if (e.key === 'Escape' && S.lightbox) { S.lightbox = null; return render() }
  if (S.view !== 'calls' || !S.calls || !S.calls.length || /input/i.test(e.target.tagName)) return
  if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return
  e.preventDefault()
  const i = S.calls.findIndex((c) => c.name === S.selected)
  const n = Math.min(S.calls.length - 1, Math.max(0, i + (e.key === 'ArrowDown' ? 1 : -1)))
  go(`call/${S.calls[n].name}/${S.tab}`)
})

window.addEventListener('resize', () => { if (S.view === 'calendar') render(); else placeDetail() })

window.addEventListener('hashchange', async () => {
  route()
  render()
  if (S.view === 'calendar' && !S.cal.data) { await loadCalendar(); render() }
  if (S.view === 'calls' && S.selected && !S.call) { await loadCall(S.selected); render() }
})

// ── refresh: cheap local polling (the files change under us during a call) ─────────────────
let ticks = 0
async function tick() {
  ticks++
  const before = JSON.stringify(S.state)
  await loadState()
  const changed = JSON.stringify(S.state) !== before
  if (changed || ticks % 3 === 0) await loadCalls()
  if (S.view === 'today' && ticks % 15 === 0) await loadAgenda()
  if (S.view === 'calendar' && ticks % 30 === 0) await loadCalendar()
  const lv = S.call && S.call.live && S.call.live.status
  const live = S.selected && S.call && (S.call.status === 'recording' || S.call.status === 'transcribing' ||
    (S.call.notes_run && (S.call.notes_run.state === 'running' || S.call.notes_run.state === 'queued')) ||
    (lv && (lv.running || lv.asking || lv.queued > 0)) || S.tab === 'live')
  if (S.selected && (live || changed)) await loadCall(S.selected)
  // Typing a question doesn't hold back the live notes (its text and caret survive a render); a
  // text selection, an open menu or a focused search field does (until it ends).
  const sel = window.getSelection && window.getSelection()
  const busy = document.querySelector('input:focus:not(#ask), select:focus') || (sel && !sel.isCollapsed)
  if (fingerprint() !== lastPrint && (!busy || changed)) render()
}

window.echo = {
  async refresh() { await loadCalls(); if (S.selected) await loadCall(S.selected); if (S.view === 'calendar') await loadCalendar(true); render() },
  openCall(name) { go(`call/${name}/notes`) },
  async selftest() {
    const out = { selftest: true, base: location.href }
    try {
      out.state = (await fetch('api/state')).status
      const calls = await get('api/calls'); out.calls = calls.length
      if (calls.length) { const d = await get('api/call/' + encodeURIComponent(calls[0].name)); out.segments = (d.segments || []).length; out.notes = d.notes ? d.notes.length : 0 }
      out.forbidden = (await fetch('file/x/notes.md')).status
      out.rendered = !!document.querySelector('.shell')
      out.ok = out.state === 200 && out.forbidden === 403 && out.rendered
    } catch (e) { out.ok = false; out.error = String(e) }
    send(out)
  },
}

;(async () => {
  route()
  render()
  await Promise.all([loadState(), loadCalls(), loadAgenda()])
  if (S.view === 'calls' && !S.selected && S.calls && S.calls.length) S.selected = S.calls[0].name, history.replaceState(null, '', `#call/${S.selected}/notes`)
  if (S.selected) await loadCall(S.selected)
  if (S.view === 'calendar') await loadCalendar()
  render()
  setInterval(tick, 2000)
})()
