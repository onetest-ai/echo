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

function markdown(md, ctx = {}) {
  const lines = String(md).replace(/\r\n?/g, '\n').split('\n')
  const out = []
  let para = []
  const flush = () => { if (para.length) { out.push(`<p>${inline(para.join(' '), ctx)}</p>`); para = [] } }
  let i = 0
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
  try { S.call = await get('api/call/' + encodeURIComponent(name)) } catch (e) { S.call = { error: String(e) } }
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
      : `<div class="status-card"><div class="status-line"><b>${st.in_call ? 'Teams call in progress' : 'Not recording'}</b><span>${st.in_call ? 'It is not being recorded' : 'Echo records only the calls you choose'}</span></div><button class="btn primary" data-cmd="record">${st.in_call ? 'Record this call' : 'Record next call'}</button></div>`
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
  const tabs = [
    ['notes', 'Notes', ''],
    ['transcript', 'Transcript', ''],
    chat.length ? ['chat', 'Chat', chat.length] : null,
    frames.length ? ['screens', 'Screens', frames.length] : null,
    ['actions', 'Actions', actions ? actions.length : ''],
  ].filter(Boolean)
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

function tabBody(c, segs, chat, frames, actions) {
  if (S.tab === 'transcript') {
    if (!segs.length) return c.status === 'recording'
      ? `<p style="color:var(--text-3)">Listening. The live transcript appears here as people speak.</p>`
      : `<p style="color:var(--text-3)">${c.status === 'transcribing' ? 'Transcribing on this computer. This takes about a quarter of the call length.' : 'No transcript for this call.'}</p>`
    const q = S.find.trim().toLowerCase()
    const turns = []
    for (const s of segs) {
      const last = turns[turns.length - 1]
      if (last && last.who === speaker(s) && s.start - last.end < 8) { last.text += ' ' + s.text; last.end = s.end }
      else turns.push({ who: speaker(s), me: speaker(s) === 'Me', start: s.start, end: s.end, lang: s.lang, text: s.text })
    }
    const jumpIdx = S.jump == null ? -1 : Math.max(0, turns.findIndex((t) => t.end >= S.jump - 2))
    return `<div class="from-bar"><input class="search" id="find" placeholder="Find in transcript" value="${esc(S.find)}" style="max-width:280px" aria-label="Find in transcript"></div>
      <div class="transcript">${turns.map((t, i) => {
        const hit = (q && t.text.toLowerCase().includes(q)) || i === jumpIdx
        return `<div class="turn ${t.me ? 'me' : 'remote'}${hit ? ' hit' : ''}" ${i === jumpIdx ? 'id="jump"' : ''}>
          <time>${clock(t.start)}</time>
          <div><div class="who">${esc(t.who)}${t.lang ? `<span class="lang">${esc(t.lang)}</span>` : ''}</div><p>${esc(t.text)}</p></div></div>`
      }).join('')}</div>`
  }
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
  if (c.status === 'recording') return `<p style="color:var(--text-3)">Notes are written after the call. Follow it live in the Transcript tab.</p>`
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
  if (run && run.state === 'running') {
    return box('Writing notes…', `<p>Claude Code is writing the notes and action items with <b>${esc(run.model || n.model || 'sonnet')}</b>. It takes a minute or two; you can close this window.</p>`, log, true)
  }
  if (c.status === 'recording' || c.status === 'transcribing') return box(heading, `<p>${esc(lead)}</p>`, '')
  const why = {
    needs_login: ['Claude Code needs you to sign in', run && run.message],
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
  return box(heading, `<p>${esc(lead)}</p><p class="alt">Claude Code writes them on this computer with <b>${esc(n.model || 'sonnet')}</b> (menu → Claude Code → Notes model).</p>`,
    `<button class="btn primary" data-cmd="write-notes">${label}</button>${log}`) + session
}

// ── calendar: today, or a rolling week (yesterday, today, 5 days ahead) ───────────────────
const DAY_MS = 86400000
const ymd = (d) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
const midnight = (d) => { const x = new Date(d); x.setHours(0, 0, 0, 0); return x }
function calRange() {
  const today = midnight(new Date())
  if (S.cal.mode === 'day') return { start: new Date(today.getTime() + S.cal.offset * DAY_MS), days: 1 }
  return { start: new Date(today.getTime() + (S.cal.offset * 7 - 1) * DAY_MS), days: 7 }
}
async function loadCalendar(fresh) {
  const { start, days } = calRange()
  S.cal.loading = true
  try {
    S.cal.data = await get(`api/calendar?start=${ymd(start)}&days=${days}${fresh ? '&fresh=1' : ''}`)
  } catch (e) { S.cal.data = { error: String(e), events: [], calls: [] } }
  S.cal.loading = false
}
const sameDay = (a, b) => midnight(a).getTime() === midnight(b).getTime()

/** Side-by-side lanes for overlapping blocks in one day column. */
function lanes(items) {
  items.sort((a, b) => a.s - b.s || b.end - a.end)
  const cols = []
  let group = [], groupEnd = 0
  const flush = () => { const n = Math.max(1, ...group.map((x) => x.lane + 1)); group.forEach((x) => (x.lanes = n)); group = [] }
  for (const it of items) {
    if (group.length && it.s >= groupEnd) { flush(); cols.length = 0 }
    let lane = cols.findIndex((end) => end <= it.s)
    if (lane < 0) { lane = cols.length; cols.push(0) }
    cols[lane] = it.end
    it.lane = lane
    group.push(it)
    groupEnd = Math.max(groupEnd, it.end)
  }
  if (group.length) flush()
  return items
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
  const dayList = Array.from({ length: days }, (_, i) => new Date(start.getTime() + i * DAY_MS))
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
  // Layout end: at least 20 min, so a short meeting is still a readable block.
  blocks.forEach((b) => lanes(b.map((x) => Object.assign(x, { end: Math.max(x.real, x.s + 20 * 60000) }))))

  // The whole day, scrolling; an hour is sized so the working day (08:00–19:00) fills the
  // window (min 40 px).
  const h0 = 0, h1 = 24
  const reserved = 112 + (allday.some((a) => a.length) ? 30 : 0) + (note ? 34 : 0)
  const HOUR = Math.max(40, Math.floor((window.innerHeight - reserved) / 11))
  const cols = `grid-template-columns: repeat(${days}, minmax(0, 1fr))`
  const dayHead = `<div class="days" style="${cols}">${dayList.map((x) => `<div class="${sameDay(x, now) ? 'today' : ''}">${esc(x.toLocaleDateString([], { weekday: 'short' }))}<b>${x.getDate()}</b></div>`).join('')}</div>`
  const alldayRow = allday.some((a) => a.length) ? `<div class="allday" style="${cols}">${allday.map((a) => `<div>${a.map((e) => `<span title="${esc(e.subject)}">${esc(e.subject)}</span>`).join('')}</div>`).join('')}</div>` : ''
  const hourLabels = Array.from({ length: h1 - h0 }, (_, i) => `<span style="top:${i * HOUR}px">${i === 0 ? '' : String(h0 + i).padStart(2, '0') + ':00'}</span>`).join('')
  const top = (t) => ((t - h0 * 3600000) / 3600000) * HOUR
  const colHtml = dayList.map((day, di) => {
    const base = day.getTime()
    const lines = Array.from({ length: h1 - h0 }, (_, i) => `<div class="hourline" style="top:${i * HOUR}px"></div>`).join('')
    const nowLine = sameDay(day, now) ? `<div class="now" style="top:${top(now.getTime() - base)}px"></div>` : ''
    const items = blocks[di].map((x) => {
      const y = Math.max(0, top(x.s - base)), h = Math.max(22, top(x.end - base) - top(x.s - base) - 2)
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
  else if (st.recordable && e.scheduled) acts = `<button class="btn" data-unschedule="${esc(e.id)}">Don't record</button><p class="why">Echo arms itself a minute before it starts.</p>`
  else if (st.recordable) acts = `${st.live && e.join_url ? `<button class="btn primary" data-join="${esc(e.id)}">Join and record</button>` : ''}<button class="btn${st.live && e.join_url ? '' : ' primary'}" data-schedule="${esc(e.id)}">${st.live ? 'Record it' : 'Record when it starts'}</button>`
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
  const t = e.target.closest('[data-go],[data-call],[data-tab],[data-cmd],[data-at],[data-toggle],[data-frame],[data-close],[data-copy],[data-arm],[data-join],[data-ev],[data-calmode],[data-calnav],[data-calclose],[data-schedule],[data-unschedule],a[data-ext]')
  if (!t) return
  if (t.dataset.ext) { e.preventDefault(); send({ cmd: 'open_url', url: t.getAttribute('href') }); return }
  if (t.dataset.go) return go(t.dataset.go === 'calls' ? (S.selected ? `call/${S.selected}/${S.tab}` : 'calls') : `calendar/${S.cal.mode}`)
  if (t.dataset.ev !== undefined) { S.cal.selected = S.cal.selected === t.dataset.ev ? null : t.dataset.ev; return render() }
  if (t.dataset.calclose) { S.cal.selected = null; return render() }
  if (t.dataset.calmode) { S.cal.mode = t.dataset.calmode; S.cal.offset = 0; S.cal.data = null; go(`calendar/${S.cal.mode}`); return loadCalendar().then(render) }
  if (t.dataset.calnav !== undefined) { const n = Number(t.dataset.calnav); S.cal.offset = n === 0 ? 0 : S.cal.offset + n; S.cal.data = null; render(); return loadCalendar().then(render) }
  if (t.dataset.schedule) { send({ cmd: 'schedule', id: t.dataset.schedule }); const e = (S.cal.data.events || []).find((x) => x.id === t.dataset.schedule); if (e) e.scheduled = true; return render() }
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
  if (t.dataset.arm !== undefined) return send({ cmd: 'arm_meeting', id: t.dataset.arm })
  if (t.dataset.join !== undefined) return send({ cmd: 'join_meeting', id: t.dataset.join })
  switch (t.dataset.cmd) {
    case 'record': return send({ cmd: 'record' })
    case 'stop': return send({ cmd: 'stop' })
    case 'reveal': return send({ cmd: 'reveal', call: S.selected })
    case 'show-log': return send({ cmd: 'show_log', call: S.selected })
    case 'write-notes':
      send({ cmd: 'write_notes', call: S.selected })
      if (S.call) { S.call.notes_run = { state: 'running', model: (S.state.notes || {}).model }; render() }
      return
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
})

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
  const live = S.selected && S.call && (S.call.status === 'recording' || S.call.status === 'transcribing' ||
    (S.call.notes_run && S.call.notes_run.state === 'running'))
  if (S.selected && (live || changed)) await loadCall(S.selected)
  if (fingerprint() !== lastPrint && (!document.querySelector('input:focus') || changed)) render()
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
