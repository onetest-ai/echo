// ── Settings → Activity (#859): what Echo ran, step by step, from its traces (`telemetry`, `/api/activity`): the last
// days' runs, tokens and cost by day and by job, the runs themselves; a run's timeline (`/api/activity/trace`) — Echo's
// steps, Claude Code's requests and tools, requests to other services — with what went in and came out. The chosen run
// or step is in the right panel.

S.ac = { days: 7, data: null, at: 0, show: 'all', sel: null, trace: null, traces: {}, step: null, export: null, exportOpen: false, form: null }

// ── activity: words, sums and the timeline's rows (no DOM: `activity.test.mjs`)
const AC_JOBS = { thread_sync: 'Thread sync', minutes: 'Minutes', live_notes: 'Live notes', needs: 'Needs you now', prep: 'Prep', brain_reader: 'Intake', fetch: 'Mail & chat fetch', yolo: 'Yolo', plugin_run: 'A plugin’s Claude run' }
const AC_APPS = { calendar_sync: 'Calendar sync', transcript_scan: 'Teams transcripts' }
/** The chart's groups, each its colour (`ac-k1`…): the three biggest jobs, then the rest. */
const AC_GROUPS = [['thread_sync', 'Thread sync'], ['minutes', 'Minutes'], ['live_notes', 'Live notes'], ['other', 'Other']]
const acGroup = (t) => (AC_GROUPS.some(([k]) => k === t.job) ? t.job : 'other')
/** A trace's name as the user reads it: its job's, else what it was. */
function acLabel(t) {
  const name = String(t.name || '')
  if (t.job && name.startsWith('job.')) return Object.hasOwn(AC_JOBS, t.job) ? AC_JOBS[t.job] : String(t.job)
  if (name === 'claude.run') return 'Claude Code run'
  if (name === 'model.request') return `Model call${t.model ? ` · ${t.model}` : ''}`
  if (name === 'http.request') return 'Request to another service'
  if (name === 'transcription') return 'Transcription'
  if (name.startsWith('app.')) return Object.hasOwn(AC_APPS, name.slice(4)) ? AC_APPS[name.slice(4)] : name.slice(4)
  if (name.startsWith('cli.')) return `echoctl ${name.slice(4)} failed`
  return name
}
/** The filters above the runs: which kinds each keeps. */
const AC_SHOW = [['all', 'All'], ['failed', 'Failed'], ['jobs', 'Jobs'], ['requests', 'Requests'], ['commands', 'Commands']]
function acShown(traces, show) {
  const jobs = ['job', 'claude', 'model', 'transcription']
  return (traces || []).filter((t) => show === 'failed' ? t.ok === false
    : show === 'jobs' ? jobs.includes(t.kind)
    : show === 'requests' ? ['request', 'app'].includes(t.kind)
    : show === 'commands' ? t.kind === 'command'
    : true)
}
/** The facts row: runs (requests and failed commands aside), how many failed, tokens and how many cached, cost,
 *  requests to other services. */
function acSummary(traces) {
  const s = { runs: 0, failed: 0, tokens: 0, cached: 0, cost: 0, requests: 0 }
  for (const t of traces || []) {
    if (!['request', 'app', 'command'].includes(t.kind)) { s.runs++; if (t.ok === false) s.failed++ }
    s.tokens += t.tokens || 0
    s.cached += t.cached || 0
    s.cost += t.cost_usd || 0
    s.requests += t.requests || 0
  }
  return s
}
const acPad = (n) => String(n).padStart(2, '0')
const acDayKey = (d) => `${d.getFullYear()}-${acPad(d.getMonth() + 1)}-${acPad(d.getDate())}`
const AC_WD = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
const AC_MON = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
/** The periods Activity shows: days, and the chip's word. */
const AC_PERIODS = [[1, 'Today'], [7, '7 days'], [30, '30 days'], [90, '90 days']]
/** Tokens a day for the last `days` days to `now`, oldest first: `[{key, label, total, by: {group: tokens}}]`; past a
 *  week, a label on each Monday only (its date); past a month, on each month's first day (its month). None within a few
 *  days of today's. */
function acDays(traces, now, days) {
  const out = []
  for (let i = days - 1; i >= 0; i--) {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - i)
    const label = i === 0 ? 'Today' : days <= 7 ? AC_WD[d.getDay()]
      : days <= 31 ? (d.getDay() === 1 && i > 2 ? `${d.getDate()} ${AC_MON[d.getMonth()]}` : '')
      : d.getDate() === 1 && i > 6 ? AC_MON[d.getMonth()] : ''
    out.push({ key: acDayKey(d), label, total: 0, by: {} })
  }
  const byKey = new Map(out.map((x) => [x.key, x]))
  for (const t of traces || []) {
    // By the day it ended, as Echo files it.
    const day = byKey.get(acDayKey(new Date(t.end || t.start)))
    if (!day || !t.tokens) continue
    const g = acGroup(t)
    day.by[g] = (day.by[g] || 0) + t.tokens
    day.total += t.tokens
  }
  return out
}
/** The table by job: a row a job (or kind of run), its runs, failed, tokens, cost and models; most tokens first. */
function acByJob(traces) {
  const rows = new Map()
  for (const t of traces || []) {
    if (t.kind === 'command' || t.kind === 'request') continue
    const key = t.job || t.name
    const r = rows.get(key) || { key, label: acLabel(t), runs: 0, failed: 0, tokens: 0, cost: 0, models: [] }
    r.runs++
    if (t.ok === false) r.failed++
    r.tokens += t.tokens || 0
    r.cost += t.cost_usd || 0
    for (const m of String(t.model || '').split(', ').filter(Boolean)) if (!r.models.includes(m)) r.models.push(m)
    rows.set(key, r)
  }
  return [...rows.values()].sort((a, b) => b.tokens - a.tokens || b.runs - a.runs || a.label.localeCompare(b.label))
}
/** Tokens in few characters: 950, 12.4K, 6.4M. */
const acNum = (v) => { const n = Number(v) || 0; return !n ? '—' : n >= 1e6 ? `${(n / 1e6).toFixed(n >= 1e7 ? 0 : 1)}M` : n >= 1e3 ? `${(n / 1e3).toFixed(n >= 1e4 ? 0 : 1)}K` : String(n) }
const acMoney = (v) => { const c = Number(v) || 0; return c ? `$${c.toFixed(2)}` : '—' }
/** How long, in few characters. */
const acTook = (ms) => ms == null ? '' : ms < 1000 ? `${Math.round(ms)} ms` : ms < 60000 ? `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)} s` : `${Math.floor(ms / 60000)} min ${Math.round((ms % 60000) / 1000)} s`
/** A tick of the timeline's axis: seconds with one decimal under a minute. */
const acTick = (ms) => (ms < 60000 ? `${(ms / 1000).toFixed(ms < 10000 ? 1 : 0)} s` : acTook(ms))
/** When a run started, from `now`: "14:42" today, "Mon 14:42" this week, else "3 Nov 14:42". */
function acWhen(iso, now) {
  const d = new Date(iso), tm = `${acPad(d.getHours())}:${acPad(d.getMinutes())}`
  if (d.toDateString() === now.toDateString()) return tm
  return now - d < 6 * 864e5 ? `${AC_WD[d.getDay()]} ${tm}` : `${d.getDate()} ${AC_MON[d.getMonth()]} ${tm}`
}
/** When a run started, to the second: "Today 14:42:06", else "Mon 18 Nov 14:42:06". */
function acWhenFull(iso, now) {
  const d = new Date(iso), tm = `${acPad(d.getHours())}:${acPad(d.getMinutes())}:${acPad(d.getSeconds())}`
  return d.toDateString() === now.toDateString() ? `Today ${tm}` : `${AC_WD[d.getDay()]} ${d.getDate()} ${AC_MON[d.getMonth()]} ${tm}`
}
/** A run's line in the list: its label, then what it did or why it failed. */
function acLine(t) {
  const what = t.ok === false ? (t.error || 'failed') : t.summary || (t.runs ? `${t.runs} Claude Code run${t.runs === 1 ? '' : 's'}` : t.requests ? `${t.requests} request${t.requests === 1 ? '' : 's'}` : '')
  return what ? `${acLabel(t)} · ${String(what).split('\n')[0]}` : acLabel(t)
}
/** A span's tokens: Echo's Claude Code run's, Claude Code's request's, a model call's. */
function acSpanTokens(s) {
  const a = s.attrs || {}, n = (k) => Number(a[k]) || 0
  if (s.name === 'claude.run') return n('input') + n('output') + n('cache_read') + n('cache_write')
  if (s.name === 'claude_code.llm_request') return n('input_tokens') + n('output_tokens') + n('cache_read_tokens') + n('cache_creation_tokens')
  if (s.name === 'model.request' && a.usage) return (Number(a.usage.input_tokens) || 0) + (Number(a.usage.output_tokens) || 0)
  return 0
}
/** A step's name on the timeline. */
function acStepLabel(s) {
  const a = s.attrs || {}, name = String(s.name || '')
  if (name.startsWith('job.')) return Object.hasOwn(AC_JOBS, name.slice(4)) ? AC_JOBS[name.slice(4)] : name.slice(4)
  if (name === 'claude.run') {
    const head = Object.values(a.files || {}).map((t) => String(t).split('\n')[0]).find((l) => l.startsWith('# Echo: '))
    return head ? `Claude Code · ${head.slice(8)}` : `Claude Code${a.model ? ` · ${a.model}` : ''}`
  }
  if (name === 'claude_code.llm_request') return `Request · ${a.model || a['gen_ai.request.model'] || 'model'}`
  if (name === 'claude_code.tool') return `${a.tool_name || 'Tool'}`
  if (name === 'http.request') return `${a.method || 'GET'} ${a.host || ''}${a.path || ''}`
  if (name === 'model.request') return `Model call · ${a.model || ''}`
  if (name.startsWith('app.')) return Object.hasOwn(AC_APPS, name.slice(4)) ? AC_APPS[name.slice(4)] : name.slice(4)
  if (name.startsWith('cli.')) return `echoctl ${a.command || name.slice(4)}`
  return name
}
/** A step's colour on the timeline: `echo` (Echo's own), `run` (a Claude Code run), `req` (a model request), `tool`,
 *  `bad` (failed or refused), `out` (a request to another service). */
function acStepKind(s) {
  const a = s.attrs || {}
  const failed = a.ok === false || a.success === false || a.success === 'false' || (s.children || []).some((c) => {
    const x = c.attrs || {}
    return x.success === false || x.success === 'false' || (x.decision && x.decision !== 'accept')
  })
  if (failed) return 'bad'
  if (s.name === 'claude.run') return 'run'
  if (s.name === 'claude_code.llm_request' || s.name === 'model.request') return 'req'
  if (s.name === 'claude_code.tool') return 'tool'
  if (s.name === 'http.request') return 'out'
  return 'echo'
}
/** A run's tool calls as Echo read them in its stream, by their id: `{id: {name, what, input, result, refused}}`. */
function acTools(trace) {
  const out = {}
  const walk = (s) => {
    for (const e of s.events || []) {
      const content = (e.attrs && e.attrs.message && e.attrs.message.content) || []
      if (!Array.isArray(content)) continue
      for (const b of content) {
        if (b.type === 'tool_use' && b.id) {
          const i = b.input || {}
          out[b.id] = { name: b.name, what: String(i.command || i.file_path || i.pattern || i.skill || i.url || ''), input: i }
        }
        if (b.type === 'tool_result' && b.tool_use_id && out[b.tool_use_id]) Object.assign(out[b.tool_use_id], { result: acText(b.content), refused: !!b.is_error })
      }
    }
    ;(s.children || []).forEach(walk)
  }
  ;((trace && trace.spans) || []).forEach(walk)
  return out
}
const acToolId = (s) => (s.attrs || {})['gen_ai.tool.call.id'] || (s.attrs || {}).tool_use_id || ''
/** Claude Code's spans the timeline folds into their parent: its interaction (the run is Echo's row) and a tool's
 *  own parts (the tool's row says how it went). */
const AC_FOLDED = ['claude_code.interaction', 'claude_code.tool.blocked_on_user', 'claude_code.tool.execution']
/** A trace's timeline: its spans depth first, oldest first at each level, as rows `{id, depth, label, kind, at, took,
 *  tokens}` (ms from the trace's start), and the whole span `{start, took}`. */
function acRows(trace) {
  const spans = (trace && trace.spans) || []
  const tools = acTools(trace)
  const t = (x) => (x ? Date.parse(x) : NaN)
  const starts = [], ends = []
  const walk0 = (s) => { if (!isNaN(t(s.start))) starts.push(t(s.start)); if (!isNaN(t(s.end))) ends.push(t(s.end)); (s.children || []).forEach(walk0) }
  spans.forEach(walk0)
  const t0 = starts.length ? Math.min(...starts) : 0, t1 = ends.length ? Math.max(...ends) : t0
  const rows = []
  const walk = (s, depth) => {
    const kids = [...(s.children || [])].sort((a, b) => t(a.start) - t(b.start))
    if (AC_FOLDED.includes(s.name)) { kids.forEach((k) => walk(k, depth)); return }
    const tool = s.name === 'claude_code.tool' ? tools[acToolId(s)] : null
    const label = tool && tool.what ? `${acStepLabel(s)} · ${tool.what.length > 60 ? `${tool.what.slice(0, 60)}…` : tool.what}` : acStepLabel(s)
    rows.push({ id: s.span, depth, label, kind: tool && tool.refused ? 'bad' : acStepKind(s), at: Math.max(0, t(s.start) - t0) || 0, took: Math.max(0, t(s.end) - t(s.start)) || 0, tokens: acSpanTokens(s) })
    if (s.name !== 'claude_code.tool') kids.forEach((k) => walk(k, depth + 1))
  }
  spans.forEach((s) => walk(s, 0))
  return { rows, took: Math.max(0, t1 - t0) }
}
/** The span `id` of a trace, wherever it is. */
function acFind(trace, id) {
  let found = null
  const walk = (s) => { if (found) return; if (s.span === id) found = s; else (s.children || []).forEach(walk) }
  ;((trace && trace.spans) || []).forEach(walk)
  return found
}
/** The text of a message's content (a string, or blocks). */
const acText = (c) => typeof c === 'string' ? c : Array.isArray(c) ? c.map((b) => b.text || (b.type === 'image' ? '[a picture]' : '')).filter(Boolean).join('\n') : ''
/** What went in and came out of a step, in order: `[{k, note, text, mono, bad}]` (each text as it is: the page escapes
 *  it). */
function acDetails(s, trace) {
  const a = s.attrs || {}, out = []
  if (s.name === 'claude_code.tool') {
    const tool = acTools(trace)[acToolId(s)]
    if (tool) {
      out.push({ k: 'Tool', note: tool.name, text: tool.what || JSON.stringify(tool.input || {}), mono: true })
      if (tool.result != null) out.push({ k: tool.refused ? 'Refused' : 'Result', text: tool.result, mono: !tool.refused, bad: tool.refused })
    }
  }
  for (const [k, v] of Object.entries(a.files || {})) out.push({ k: 'Instructions', note: k, text: String(v), mono: true })
  if (s.name === 'model.request') {
    if (a.system) out.push({ k: 'Instructions', text: String(a.system), mono: true })
    out.push({ k: 'In', text: acText(a.input), mono: true })
    if (a.output) out.push({ k: 'Answer', text: acText(a.output) })
  }
  if (s.name === 'http.request' && a.query) out.push({ k: 'Query', text: String(a.query), mono: true })
  if (a.error) out.push({ k: 'Error', text: String(a.error), bad: true })
  for (const e of s.events || []) {
    const x = e.attrs || {}
    if (e.name === 'claude.input') {
      let text = String(x.text || '')
      try { const m = JSON.parse(text); text = acText(m.message && m.message.content) || text } catch { /* plain text */ }
      out.push({ k: 'In', text, mono: true })
    } else if (e.name === 'claude.assistant') {
      for (const b of (x.message && x.message.content) || []) {
        if (b.type === 'text' && String(b.text || '').trim()) out.push({ k: 'Claude', text: b.text })
        if (b.type === 'tool_use') out.push({ k: 'Tool', note: b.name, text: b.input && (b.input.command || b.input.file_path || b.input.pattern) ? String(b.input.command || b.input.file_path || b.input.pattern) : JSON.stringify(b.input || {}), mono: true })
      }
    } else if (e.name === 'claude.user') {
      for (const b of (x.message && x.message.content) || []) {
        if (b.type === 'tool_result' && b.is_error) out.push({ k: 'Refused', text: acText(b.content), bad: true })
      }
    } else if (e.name && e.name.startsWith('claude.result')) {
      if (x.result) out.push({ k: x.is_error ? 'Failed' : 'Result', text: String(x.result), bad: !!x.is_error })
    }
  }
  return out
}
// end of the activity block

async function loadActivity() {
  S.ac.at = Date.now()
  try { S.ac.data = await get(`api/activity?days=${S.ac.days}`) } catch { /* app restarting */ }
  if (S.view === 'settings' && S.setSection === 'activity') render()
}
/** A run's trace, read once — again while it may still be filling (Claude Code sends its spans a few seconds after its
 *  run: a trace read within 2 min of its end is read again on the next open). The last 20 read are kept. */
async function loadActivityTrace(id) {
  const had = S.ac.traces[id], t = acTrace(id)
  const settled = had && !had.error && (!t || had.at - Date.parse(t.end) > 120000)
  if (settled) return
  let v = null
  try { v = await get(`api/activity/trace?id=${encodeURIComponent(id)}`) } catch { /* app restarting */ }
  S.ac.traces[id] = v ? { ...v, at: Date.now() } : { error: "This run can't be read now." }
  const ids = Object.keys(S.ac.traces)
  for (const old of ids.slice(0, Math.max(0, ids.length - 20))) if (old !== id) delete S.ac.traces[old]
  render()
}
// Read when the section is entered, then every 30 s while it is open (a month or more: every 5 min).
setInterval(() => {
  if (S.view !== 'settings' || S.setSection !== 'activity' || document.hidden) return
  if (!S.ac.data || Date.now() - S.ac.at > (S.ac.days > 7 ? 300000 : 30000)) { loadActivity(); loadActivityExport() }
  // A test send's outcome, as soon as it is there.
  if (S.ac.exportOpen && S.ac.export && S.ac.export.test && S.ac.export.test.busy) loadActivityExport()
}, 1000)

const acDot = (ok) => `<span class="ac-dot ${ok === false ? 'bad' : 'ok'}"${ok === false ? ' aria-label="failed"' : ''}></span>`
const acTrace = (id) => ((S.ac.data && S.ac.data.traces) || []).find((t) => t.id === id)

/** Settings → Activity: the list, a run's timeline, or where traces are sent. */
function activitySettings() {
  const d = S.ac.data
  if (S.ac.exportOpen) return activityExport()
  if (!d) return `<div class="ac"><div class="skeleton" style="padding:20px">${'<i></i>'.repeat(5)}</div></div>`
  return S.ac.trace && acTrace(S.ac.trace) ? activityTimeline(acTrace(S.ac.trace)) : activityList(d)
}
async function loadActivityExport() {
  try { S.ac.export = await get('api/activity/export') } catch { /* app restarting */ }
  // The form follows what is kept, unless the user is changing it.
  if (!S.ac.formDirty) S.ac.form = null
  if (S.view === 'settings' && S.setSection === 'activity') render()
}
/** Where traces are sent, in words: "Sending to Langfuse", or nothing when off. */
const acSending = (x) => (x && x.on ? `Sending to ${x.kind === 'langfuse' ? 'Langfuse' : 'your collector'}${x.error ? ' · failing' : ''}` : '')
/** The form as the user edits it, from what is kept (the secret never: only whether one is). */
function acForm() {
  if (!S.ac.form) {
    const x = S.ac.export || {}
    S.ac.form = { kind: x.kind || 'langfuse', endpoint: x.endpoint || (x.kind === 'otlp' ? '' : 'https://cloud.langfuse.com'), public_key: x.public_key || '', secret_key: '', headers: '', text: !!x.text }
  }
  return S.ac.form
}
function activityExport() {
  const x = S.ac.export, f = acForm()
  const head = `<nav class="ac-crumb"><button class="link-btn" data-ac-export-close>Activity</button><span>›</span><span>Send traces elsewhere</span></nav>
    <header class="ac-head"><div><h2>Send traces elsewhere</h2><div class="set-sub">Off until you set it up. Echo sends each run once it ended — to your Langfuse, or any OpenTelemetry (OTLP) collector. Never your account’s identity; what went in and came out only if you say so.</div></div></header>`
  if (!x) return `<div class="ac">${head}<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(3)}</div></div>`
  const inp = (key, label, attrs = '') => `<input class="ac-in" data-ac-f="${key}" value="${esc(f[key])}" aria-label="${esc(label)}" ${attrs}>`
  const lf = f.kind === 'langfuse'
  const rows = [
    setRow('Send to', '', `<div class="seg" role="group" aria-label="Send to">${[['langfuse', 'Langfuse'], ['otlp', 'OTLP collector']].map(([v, l]) => `<button data-ac-kind="${v}" aria-pressed="${f.kind === v}">${l}</button>`).join('')}</div>`),
    setRow(lf ? 'Langfuse host' : 'Collector', lf ? 'Cloud: https://cloud.langfuse.com (EU) or https://us.cloud.langfuse.com; or your own.' : 'Its traces URL, or its base (/v1/traces is added). https://…, or http:// on this computer.', inp('endpoint', 'Address', 'placeholder="https://…"')),
    ...(lf ? [
      setRow('Public key', '', inp('public_key', 'Public key', 'placeholder="pk-lf-…"')),
      setRow('Secret key', x.secret_key_set && x.kind === 'langfuse' ? 'Kept — type a new one to change it.' : 'Kept on this computer, readable only by you.', inp('secret_key', 'Secret key', `type="password" placeholder="${x.secret_key_set && x.kind === 'langfuse' ? '•••••• kept' : 'sk-lf-…'}"`)),
    ] : [
      setRow('Headers', x.headers && x.headers.length && x.kind === 'otlp' ? `Kept: ${esc(x.headers.join(', '))} — type them again to change them.` : 'One a line, Name: value (an API key for your collector).', `<textarea class="ac-in" data-ac-f="headers" rows="2" aria-label="Headers" placeholder="Authorization: Bearer …">${esc(f.headers)}</textarea>`),
    ]),
    setRow('What went in and came out', 'Prompts, replies, tool calls and results, instructions. Off: only what ran, how long, tokens and cost.', `<button class="sw${f.text ? ' on' : ''}" role="switch" aria-checked="${f.text}" aria-label="Send what went in and came out" data-ac-text></button>`),
  ]
  const t = x.test || {}
  const state = x.on ? `<p class="set-sub">${esc(acSending(x))}${x.last_sent ? ` · last sent ${esc(acWhen(x.last_sent, new Date()))}` : ''}${x.sent ? ` · ${x.sent} sent` : ''}</p>${x.error ? `<p class="error">${esc(x.error)}</p>` : ''}` : '<p class="set-sub">Not sending.</p>'
  const test = t.busy ? '<span class="set-sub">Sending…</span>' : t.ok ? '<span class="set-ok">Sent ✓</span>' : t.error ? `<span class="error">${esc(t.error)}</span>` : ''
  return `<div class="ac ac-export">${head}${setCard('', rows)}
    <div class="ac-btns"><button class="btn primary" data-ac-export-save>${x.on ? 'Save' : 'Turn on'}</button>${x.on ? '<button class="btn" data-ac-export-off>Turn off</button><button class="btn" data-ac-export-test>Send a test</button>' : ''}${test}</div>${state}</div>`
}
/** Keep the form as it is, sending on (`on`) or off. */
function acExportSave(on) {
  const f = acForm()
  const settings = { on, kind: f.kind, endpoint: f.endpoint, public_key: f.public_key, secret_key: f.secret_key, text: f.text,
    headers: String(f.headers || '').split('\n').map((h) => h.trim()).filter(Boolean), keep_headers: !String(f.headers || '').trim() }
  send({ cmd: 'activity_export', settings })
  S.ac.formDirty = false
  setTimeout(loadActivityExport, 400)
}
document.addEventListener('input', (e) => {
  const k = e.target && e.target.dataset && e.target.dataset.acF
  if (k) { acForm()[k] = e.target.value; S.ac.formDirty = true }
})
/** The list's sums, kept until the data or the day changes (Settings draws again every few seconds). */
let acMemo = { data: null, day: '', sums: null }
function acSums(d, now) {
  const day = acDayKey(now)
  if (acMemo.data !== d || acMemo.day !== day) {
    const traces = d.traces || []
    acMemo = { data: d, day, sums: { s: acSummary(traces), days: acDays(traces, now, S.ac.days), jobs: acByJob(traces) } }
  }
  return acMemo.sums
}
/** A trace's timeline rows, kept for the trace read. */
const acRowsMemo = new WeakMap()
const acRowsOf = (tr) => { if (!acRowsMemo.has(tr)) acRowsMemo.set(tr, acRows(tr)); return acRowsMemo.get(tr) }
function activityList(d) {
  const now = new Date(), traces = d.traces || []
  const { s, days, jobs: byJob } = acSums(d, now)
  const max = Math.max(1, ...days.map((x) => x.total))
  const bars = days.map((x) => `<div class="ac-day" title="${esc(`${x.label}: ${acNum(x.total)} tokens`)}"><div class="ac-stack">${AC_GROUPS.map(([g], i) => x.by[g] ? `<i class="ac-k${i + 1}" style="height:${Math.max(2, Math.round((x.by[g] / max) * 70))}px"></i>` : '').join('')}</div><span>${esc(x.label)}</span></div>`).join('')
  const legend = AC_GROUPS.map(([, l], i) => `<span class="ac-lg"><i class="ac-k${i + 1}"></i>${esc(l)}</span>`).join('')
  const jobs = byJob.map((r) => `<div class="ac-tr" role="row"><span>${esc(r.label)}${r.failed ? ` ${acDot(false)}` : ''}</span><span class="num">${r.runs}</span><span class="num">${acNum(r.tokens)}</span><span class="num">${acMoney(r.cost)}</span><span class="set-sub">${esc(r.models.join(', ') || '—')}</span></div>`).join('')
  const shown = acShown(traces, S.ac.show)
  const runs = shown.slice(0, 300).map((t) => `<button class="ac-row${S.ac.sel === t.id ? ' on' : ''}" data-ac-run="${esc(t.id)}">${acDot(t.ok)}<span class="set-sub num">${esc(acWhen(t.start, now))}</span><span class="ac-what">${esc(acLine(t))}</span><span class="num">${acNum(t.tokens)}</span><span class="num set-sub">${acMoney(t.cost_usd)}</span></button>`).join('')
    || `<p class="set-sub ac-none">${traces.length ? 'Nothing here.' : 'Nothing yet: runs show here as Echo runs its jobs.'}</p>`
  const chips = (list, cur, attr) => `<div class="ac-chips" role="group">${list.map(([v, l]) => `<button class="ac-chip${cur === v ? ' on' : ''}" ${attr}="${esc(String(v))}" aria-pressed="${cur === v}">${esc(l)}</button>`).join('')}</div>`
  return `<div class="ac">
    <header class="ac-head"><div><h2>Activity</h2><div class="set-sub">What Echo ran, step by step, on this computer — each run's detail kept ${d.kept_days || 7} days, its numbers ${d.numbers_days || 90} · <button class="link-btn" data-ac-export-open>${esc(acSending(S.ac.export) || 'Send traces elsewhere…')}</button></div></div>${chips(AC_PERIODS, S.ac.days, 'data-ac-days')}</header>
    <div class="ac-facts" aria-label="${esc((AC_PERIODS.find(([n]) => n === S.ac.days) || [0, ''])[1])}">
      <div><b class="num">${s.runs}</b><span>runs${s.failed ? ` · ${s.failed} failed` : ''}</span></div>
      <div><b class="num">${acNum(s.tokens)}</b><span>tokens${s.tokens ? ` · ${Math.round((s.cached / s.tokens) * 100)}% from cache` : ''}</span></div>
      <div><b class="num">${acMoney(s.cost)}</b><span>estimated, as Claude Code counts</span></div>
      <div><b class="num">${acNum(s.requests)}</b><span>requests to other services</span></div>
    </div>
    <section class="set-card ac-chart" aria-label="Tokens a day"><div class="ac-ch-head"><h3>Tokens a day</h3>${legend}</div><div class="ac-days${days.length > 7 ? ' many' : ''}" style="grid-template-columns:repeat(${days.length},minmax(0,1fr))">${bars}</div></section>
    <div class="ac-grid">
      <section class="set-card ac-jobs" aria-label="By job"><div class="ac-th" role="row"><span>Job</span><span>Runs</span><span>Tokens</span><span>Cost</span><span>Model</span></div>${jobs || '<p class="set-sub ac-none">No runs.</p>'}</section>
      <section class="set-card ac-runs" aria-label="Runs"><div class="ac-filter">${chips(AC_SHOW, S.ac.show, 'data-ac-show')}</div><div class="ac-list">${runs}</div></section>
    </div>
  </div>`
}
function activityTimeline(t) {
  const tr = S.ac.traces[t.id]
  const head = `<nav class="ac-crumb"><button class="link-btn" data-ac-back>Activity</button><span>›</span><span>${esc(acLabel(t))} · ${esc(acWhen(t.start, new Date()))}</span></nav>
    <header class="ac-head"><div><h2>${acDot(t.ok)} ${esc(acLine(t))}</h2><div class="set-sub num">${esc([acWhenFull(t.start, new Date()), acTook(t.duration_ms), t.runs ? `${t.runs} Claude Code run${t.runs === 1 ? '' : 's'}` : '', t.tokens ? `${t.tokens.toLocaleString()} tokens${t.cached ? ` (${Math.round((t.cached / t.tokens) * 100)}% cached)` : ''}` : '', t.cost_usd ? `${acMoney(t.cost_usd)} estimated` : '', t.model].filter(Boolean).join(' · '))}</div></div><button class="btn" data-copy="${esc(t.id)}">Copy id</button></header>`
  if (!tr) return `<div class="ac">${head}<div class="skeleton" style="padding:20px">${'<i></i>'.repeat(5)}</div></div>`
  if (tr.error) return `<div class="ac">${head}<p class="error">${esc(tr.error)}</p></div>`
  const { rows, took } = acRowsOf(tr)
  const pct = (ms) => (took ? Math.min(100, (ms / took) * 100) : 0)
  const ticks = [0, 0.25, 0.5, 0.75, 1].map((f) => `<span>${esc(acTick(took * f))}</span>`).join('')
  const lines = rows.map((r) => `<button class="ac-ln${S.ac.step === r.id ? ' on' : ''}" data-ac-step="${esc(r.id || '')}"><span class="ac-nm" style="padding-left:${r.depth * 14}px">${esc(r.label)}</span><span class="ac-lane"><i class="ac-${r.kind}" style="left:${pct(r.at).toFixed(2)}%;width:${Math.max(0.6, pct(r.took)).toFixed(2)}%"></i></span><span class="num set-sub">${r.tokens ? acNum(r.tokens) : ''}</span></button>`).join('')
  return `<div class="ac">${head}
    <section class="set-card ac-time" aria-label="Timeline"><div class="ac-ax"><span>Step</span><span class="ac-ticks num">${ticks}</span><span>Tokens</span></div><div class="ac-lines">${lines}</div></section>
    <p class="set-sub">Bars: Echo’s steps <b class="ac-c-echo">violet</b>, Claude Code’s runs <b class="ac-c-run">light violet</b>, model requests <b class="ac-c-req">blue</b>, tools <b class="ac-c-tool">green</b>, requests to other services <b class="ac-c-out">orange</b>, failed or refused <b class="ac-c-bad">red</b>. Click a step for what went in and came out.</p>
  </div>`
}
/** The right panel: the chosen run (the list), or the chosen step (a timeline). */
function activityPanel() {
  const t = acTrace(S.ac.trace || S.ac.sel)
  if (!t) return '<div class="wp-empty">Pick a run.</div>'
  const tr = S.ac.traces[t.id]
  if (S.ac.trace) {
    const s = tr && !tr.error && acFind(tr, S.ac.step)
    if (!s) return '<div class="wp-empty">Pick a step on the timeline.</div>'
    const a = s.attrs || {}
    const kv = [['Took', acTook(Date.parse(s.end) - Date.parse(s.start))], ['Tokens', acSpanTokens(s) ? acSpanTokens(s).toLocaleString() : ''], ['Cost', a.cost_usd ? `${acMoney(Number(a.cost_usd))} estimated` : ''],
      ['Model', a.model || a['gen_ai.request.model'] || ''], ['Profile', a.profile || ''], ['Turns', a.turns || ''], ['Telemetry', a['echo.telemetry'] === 'echo' ? 'Claude Code reports to Echo' : a['echo.telemetry'] || ''],
      ['Service', a.service || ''], ['Status', a.ok === false ? 'Failed' : '']].filter(([, v]) => v !== '' && v != null)
    const det = acDetails(s, tr).map((x) => `<div class="ac-ev"><div class="ac-k">${esc(x.k)}${x.note ? ` <span>${esc(x.note)}</span>` : ''}</div><div class="${x.mono ? 'ac-mono' : 'ac-tx'}${x.bad ? ' bad' : ''}">${esc(String(x.text || '').slice(0, 4000))}</div></div>`).join('')
    return `<div class="ac-pane"><b class="ac-pt">${esc(acStepLabel(s))}</b><div class="ac-kv">${kv.map(([k, v]) => `<span>${esc(k)}</span><div>${esc(String(v))}</div>`).join('')}</div>${det ? `<h4 class="ac-h4">What went in and came out</h4>${det}` : ''}</div>`
  }
  const steps = tr && !tr.error ? ((tr.spans[0] && tr.spans[0].children) || []).map((c) => [acStepLabel(c), acSpanTokens(c) || (c.children || []).reduce((n, k) => n + acSpanTokens(k), 0)]) : []
  const most = Math.max(1, ...steps.map(([, n]) => n))
  return `<div class="ac-pane"><b class="ac-pt">${acDot(t.ok)} ${esc(acLabel(t))}</b><div class="set-sub num">${esc([acWhen(t.start, new Date()), acTook(t.duration_ms), t.runs ? `${t.runs} Claude Code run${t.runs === 1 ? '' : 's'}` : '', t.summary].filter(Boolean).join(' · '))}</div>
    ${t.detail === false ? `<p class="set-sub">Its steps are gone: a run's detail is kept ${(S.ac.data && S.ac.data.kept_days) || 7} days, its numbers ${(S.ac.data && S.ac.data.numbers_days) || 90}.</p>` : `<div class="ac-btns"><button class="btn" data-ac-open="${esc(t.id)}">Open timeline</button><button class="btn" data-copy="${esc(t.id)}">Copy id</button></div>`}
    <div class="ac-kv">${[['Model', t.model], ['Tokens', t.tokens ? `${t.tokens.toLocaleString()}${t.cached ? ` · ${Math.round((t.cached / t.tokens) * 100)}% from cache` : ''}` : ''], ['Cost', t.cost_usd ? `${acMoney(t.cost_usd)} estimated` : ''], ['Requests', t.requests || ''], ['Error', t.ok === false ? t.error : '']].filter(([, v]) => v).map(([k, v]) => `<span>${esc(k)}</span><div>${esc(String(v))}</div>`).join('')}</div>
    ${steps.length ? `<h4 class="ac-h4">Steps</h4><div class="ac-mini">${steps.map(([l, n]) => `<span>${esc(l)}</span><span class="num set-sub">${acNum(n)}</span><i style="width:${Math.max(2, Math.round((n / most) * 100))}%"></i><span></span>`).join('')}</div>` : ''}
    ${t.detail === false ? '' : `<p class="set-sub">The prompt, every reply, tool call and result are in the timeline. Detail goes after ${(S.ac.data && S.ac.data.kept_days) || 7} days.</p>`}</div>`
}
PANEL_VIEWS.push({ id: 'activity', get label() { return S.ac.trace ? 'Step' : 'Run' }, icon: ICON('<path d="M3 12h4l3-7 4 14 3-7h4"/>'), area: 'right', when: () => S.view === 'settings' && S.setSection === 'activity' && !!(S.ac.sel || S.ac.trace), render: activityPanel })
/** Show the chosen run or step in the right panel. */
function acReveal() {
  const known = panelViews(), eff = Layout.effective(window.innerWidth, window.innerHeight, known, S.overlay)
  if (eff.narrow) S.overlay = true
  Layout.show('activity', known)
  layoutChanged()
}
document.addEventListener('click', (e) => {
  const t = e.target.closest && e.target.closest('[data-ac-days],[data-ac-show],[data-ac-run],[data-ac-open],[data-ac-back],[data-ac-step],[data-ac-export-open],[data-ac-export-close],[data-ac-kind],[data-ac-text],[data-ac-export-save],[data-ac-export-off],[data-ac-export-test]')
  if (!t) return
  const d = t.dataset
  if (d.acExportOpen !== undefined) { S.ac.exportOpen = true; S.ac.form = null; S.ac.formDirty = false; render(); return loadActivityExport() }
  if (d.acExportClose !== undefined) { S.ac.exportOpen = false; return render() }
  if (d.acKind) { const f = acForm(); if (f.kind !== d.acKind) Object.assign(f, { kind: d.acKind, endpoint: d.acKind === 'langfuse' ? 'https://cloud.langfuse.com' : '', headers: '' }); S.ac.formDirty = true; return render() }
  if (d.acText !== undefined) { acForm().text = !acForm().text; S.ac.formDirty = true; return render() }
  if (d.acExportSave !== undefined) return acExportSave(true)
  if (d.acExportOff !== undefined) return acExportSave(false)
  if (d.acExportTest !== undefined) { send({ cmd: 'activity_export_test' }); S.ac.export = { ...S.ac.export, test: { busy: true } }; render(); return setTimeout(loadActivityExport, 500) }
  if (d.acDays) { const n = Number(d.acDays); S.ac.days = AC_PERIODS.some(([x]) => x === n) ? n : 7; S.ac.data = null; render(); return loadActivity() }
  if (d.acShow) { S.ac.show = d.acShow; return render() }
  if (d.acRun) { S.ac.sel = d.acRun; acReveal(); render(); const t = acTrace(S.ac.sel); return t && t.detail !== false ? loadActivityTrace(S.ac.sel) : undefined }
  if (d.acOpen) { S.ac.trace = d.acOpen; S.ac.step = null; render(); return loadActivityTrace(S.ac.trace) }
  if (d.acBack !== undefined) { S.ac.trace = null; S.ac.step = null; return render() }
  if (d.acStep) { S.ac.step = d.acStep; acReveal(); return render() }
})
SETTINGS_BODY.activity = () => activitySettings()
